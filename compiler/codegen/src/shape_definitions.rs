//! Canonical strong layout, scan, dispatch and TypeDescriptor definitions.

use std::collections::{BTreeMap, BTreeSet};

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::TargetData;
use inkwell::types::AnyType;
use inkwell::values::{AnyValue, BasicValueEnum, GlobalValue, PointerValue, UnnamedAddress};
use scoop_lir::{
    DefinitionAtomRole, LayoutKind, ObjectDefinitionAtomId, PersistentDispatchTableId,
    PersistentLayoutId, PersistentScanId, RefScan, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole, StrongObjectSymbolSurfaceV1,
};

use crate::atom_boundaries::{GlobalAtomMaterializationV1, emit_global_atom_boundaries_v1};
use crate::type_descriptors::private_const_global;
use crate::{CodegenError, Module, SCAN_ARRAY, SCAN_SEQUENCE};

#[derive(Clone, Copy)]
pub(super) struct EmittedScanDefinitionV1<'ctx> {
    global: GlobalValue<'ctx>,
    is_empty: bool,
}

impl<'ctx> EmittedScanDefinitionV1<'ctx> {
    pub(super) fn runtime_pointer(self) -> Option<PointerValue<'ctx>> {
        if self.is_empty {
            None
        } else {
            Some(self.global.as_pointer_value())
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct EmittedDispatchDefinitionV1<'ctx> {
    global: GlobalValue<'ctx>,
    is_empty: bool,
}

impl<'ctx> EmittedDispatchDefinitionV1<'ctx> {
    pub(super) fn runtime_pointer(self) -> Option<PointerValue<'ctx>> {
        if self.is_empty {
            None
        } else {
            Some(self.global.as_pointer_value())
        }
    }
}

pub(super) struct EmittedStrongShapeDefinitionsV1<'ctx> {
    scans: BTreeMap<PersistentScanId, EmittedScanDefinitionV1<'ctx>>,
    dispatch_tables: BTreeMap<PersistentDispatchTableId, EmittedDispatchDefinitionV1<'ctx>>,
    atoms: Vec<GlobalAtomMaterializationV1<'ctx>>,
}

impl<'ctx> EmittedStrongShapeDefinitionsV1<'ctx> {
    pub(super) fn scan(
        &self,
        id: PersistentScanId,
    ) -> Result<EmittedScanDefinitionV1<'ctx>, CodegenError> {
        self.scans.get(&id).copied().ok_or_else(|| {
            CodegenError(format!(
                "strong scan definition {id} is absent from the emitted shape set"
            ))
        })
    }

    pub(super) fn record_atom(&mut self, atom: ObjectDefinitionAtomId, owner: GlobalValue<'ctx>) {
        self.atoms
            .push(GlobalAtomMaterializationV1::new(atom, owner));
    }
}

/// Emit every address-significant shape definition from the closed production
/// surface. Static-storage scans may already have been defined by their
/// registration emitter; those definitions are validated and reused.
pub(super) fn emit_strong_shape_definitions_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    surface: &StrongObjectSymbolSurfaceV1,
    module: &Module,
    type_globals: crate::type_descriptors::TypeDescriptorGlobals<'_, 'ctx>,
) -> Result<(), CodegenError> {
    let layouts = collect_layouts(module)?;
    let scans = collect_scans(module)?;
    validate_shape_plan_coverage(surface, &layouts, &scans, module)?;

    let mut emitted = EmittedStrongShapeDefinitionsV1 {
        scans: BTreeMap::new(),
        dispatch_tables: BTreeMap::new(),
        atoms: Vec::new(),
    };
    for layout in layouts {
        let definition = require_definition(
            surface,
            StrongDefinitionEntity::layout(layout),
            StrongDefinitionRole::Layout,
        )?;
        let symbol = definition.primary_symbol().symbol();
        require_absent_value(llvm, symbol.as_str(), "layout definition")?;
        let global = llvm.add_global(context.i8_type(), None, symbol.as_str());
        global.set_linkage(Linkage::External);
        global.set_constant(true);
        global.set_alignment(8);
        global.set_initializer(&context.i8_type().const_zero());
        emitted.record_atom(definition.primary_atom(), global);
    }
    for (scan, payload) in scans {
        let definition = require_definition(
            surface,
            StrongDefinitionEntity::scan(scan),
            StrongDefinitionRole::ScanProgram,
        )?;
        let (definition, newly_defined) = emit_or_reuse_scan_definition(
            context,
            llvm,
            definition.primary_symbol().symbol().as_str(),
            &payload,
        )?;
        if newly_defined {
            emitted.record_atom(
                require_definition(
                    surface,
                    StrongDefinitionEntity::scan(scan),
                    StrongDefinitionRole::ScanProgram,
                )?
                .primary_atom(),
                definition,
            );
        }
        emitted.scans.insert(
            scan,
            EmittedScanDefinitionV1 {
                global: definition,
                is_empty: !payload.contains_reference(),
            },
        );
    }

    crate::type_descriptors::emit_strong_type_descriptors_v1(
        context,
        llvm,
        surface,
        type_globals,
        module,
        &mut emitted,
    )?;
    emit_global_atom_boundaries_v1(llvm, target_data, surface, emitted.atoms)
}

pub(super) fn emit_dispatch_definition_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &StrongObjectSymbolSurfaceV1,
    table: PersistentDispatchTableId,
    values: &[PointerValue<'ctx>],
    emitted: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
) -> Result<EmittedDispatchDefinitionV1<'ctx>, CodegenError> {
    let definition = require_definition(
        surface,
        StrongDefinitionEntity::dispatch_table(table),
        StrongDefinitionRole::DispatchTable,
    )?;
    let symbol = definition.primary_symbol().symbol();
    require_absent_value(llvm, symbol.as_str(), "dispatch-table definition")?;
    let (global, is_empty) = if values.is_empty() {
        let global = llvm.add_global(context.i8_type(), None, symbol.as_str());
        global.set_initializer(&context.i8_type().const_zero());
        (global, true)
    } else {
        let value = context
            .ptr_type(inkwell::AddressSpace::default())
            .const_array(values);
        let global = llvm.add_global(value.get_type(), None, symbol.as_str());
        global.set_initializer(&value);
        (global, false)
    };
    global.set_linkage(Linkage::External);
    global.set_constant(true);
    let result = EmittedDispatchDefinitionV1 { global, is_empty };
    if emitted.dispatch_tables.insert(table, result).is_some() {
        return Err(CodegenError(format!(
            "dispatch-table definition {table} was emitted more than once"
        )));
    }
    emitted.record_atom(definition.primary_atom(), global);
    Ok(result)
}

fn collect_layouts(module: &Module) -> Result<BTreeSet<PersistentLayoutId>, CodegenError> {
    let mut layouts = BTreeSet::new();
    for identity in module
        .meta
        .layouts
        .iter()
        .map(|(_, layout)| &layout.identity)
        .chain(module.meta.arrays.iter().map(|(_, array)| &array.identity))
        .chain(
            module
                .meta
                .type_descriptors
                .iter()
                .map(|(_, descriptor)| &descriptor.instance_layout),
        )
        .chain(
            module
                .globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    scoop_lir::GlobalInit::Storage { layout, .. } => layout.local(),
                    scoop_lir::GlobalInit::StringConst { .. }
                    | scoop_lir::GlobalInit::CString { .. } => None,
                }),
        )
    {
        layouts.insert(identity.layout_record().id());
    }
    if layouts.is_empty() && !module.meta.type_descriptors.is_empty() {
        return Err(CodegenError(
            "local TypeDescriptors have no persistent layout definitions".to_string(),
        ));
    }
    Ok(layouts)
}

fn collect_scans(module: &Module) -> Result<BTreeMap<PersistentScanId, RefScan>, CodegenError> {
    let mut scans = BTreeMap::new();
    for (_, layout) in module.meta.layouts.iter() {
        let payload = match &layout.kind {
            LayoutKind::Plain { scan } | LayoutKind::Enum { scan } => scan.clone(),
            LayoutKind::Intrinsic(_) => RefScan::None,
        };
        insert_scan(&mut scans, layout.identity.scan_record().id(), payload)?;
    }
    for (_, array) in module.meta.arrays.iter() {
        insert_scan(
            &mut scans,
            array.identity.scan_record().id(),
            array.layout.instance().inline_scan().clone(),
        )?;
    }
    for (_, descriptor) in module.meta.type_descriptors.iter() {
        insert_scan(
            &mut scans,
            descriptor.instance_layout.scan_record().id(),
            descriptor.instance_shape.object_scan().clone(),
        )?;
    }
    for (_, global) in module.globals.iter() {
        if let scoop_lir::GlobalInit::Storage {
            layout: scoop_lir::StaticStorageLayout::Local(layout),
            ..
        } = &global.init
        {
            insert_scan(&mut scans, layout.scan_record().id(), global.scan.clone())?;
        }
    }
    Ok(scans)
}

fn insert_scan(
    scans: &mut BTreeMap<PersistentScanId, RefScan>,
    id: PersistentScanId,
    payload: RefScan,
) -> Result<(), CodegenError> {
    match scans.entry(id) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(payload);
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &payload => Ok(()),
        std::collections::btree_map::Entry::Occupied(_) => Err(CodegenError(format!(
            "persistent scan {id} has conflicting LIR payloads"
        ))),
    }
}

fn validate_shape_plan_coverage(
    surface: &StrongObjectSymbolSurfaceV1,
    layouts: &BTreeSet<PersistentLayoutId>,
    scans: &BTreeMap<PersistentScanId, RefScan>,
    module: &Module,
) -> Result<(), CodegenError> {
    let expected_layouts = surface
        .plans()
        .iter()
        .filter_map(|plan| match (plan.owner().kind(), plan.definition_role()) {
            (StrongDefinitionEntityKind::Layout(id), StrongDefinitionRole::Layout) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_scans = surface
        .plans()
        .iter()
        .filter_map(|plan| match (plan.owner().kind(), plan.definition_role()) {
            (StrongDefinitionEntityKind::Scan(id), StrongDefinitionRole::ScanProgram) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let actual_scans = scans.keys().copied().collect::<BTreeSet<_>>();
    if &expected_layouts != layouts {
        return Err(CodegenError(format!(
            "strong layout definition coverage mismatch: planned {}, LIR {}",
            expected_layouts.len(),
            layouts.len()
        )));
    }
    if expected_scans != actual_scans {
        return Err(CodegenError(format!(
            "strong scan definition coverage mismatch: planned {}, LIR {}",
            expected_scans.len(),
            actual_scans.len()
        )));
    }
    let expected_dispatch = surface
        .plans()
        .iter()
        .filter(|plan| plan.definition_role() == StrongDefinitionRole::DispatchTable)
        .count();
    let actual_dispatch = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| 1 + descriptor.itables.len())
        .sum::<usize>();
    if expected_dispatch != actual_dispatch {
        return Err(CodegenError(format!(
            "strong dispatch-table definition coverage mismatch: planned {expected_dispatch}, LIR {actual_dispatch}"
        )));
    }
    Ok(())
}

fn emit_or_reuse_scan_definition<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
    scan: &RefScan,
) -> Result<(GlobalValue<'ctx>, bool), CodegenError> {
    if llvm.get_function(symbol).is_some() {
        return Err(CodegenError(format!(
            "scan definition `{symbol}` collides with an LLVM function"
        )));
    }
    if let Some(global) = llvm.get_global(symbol) {
        validate_existing_scan(context, global, symbol, scan)?;
        return Ok((global, false));
    }
    let words = scan_words(context, llvm, symbol, scan)?;
    let value = context.i64_type().const_array(&words);
    let global = llvm.add_global(value.get_type(), None, symbol);
    global.set_linkage(Linkage::External);
    global.set_constant(true);
    global.set_alignment(8);
    global.set_initializer(&value);
    Ok((global, true))
}

fn validate_existing_scan(
    context: &Context,
    global: GlobalValue<'_>,
    symbol: &str,
    scan: &RefScan,
) -> Result<(), CodegenError> {
    let expected_words = match scan {
        RefScan::None => vec![context.i64_type().const_zero()],
        RefScan::References(offsets) => {
            std::iter::once(context.i64_type().const_int(offsets.len() as u64, false))
                .chain(
                    offsets
                        .iter()
                        .map(|offset| context.i64_type().const_int(*offset, false)),
                )
                .collect()
        }
        RefScan::Sequence(_) | RefScan::Array { .. } => {
            return Err(CodegenError(format!(
                "recursive scan definition `{symbol}` was defined before canonical shape emission"
            )));
        }
    };
    let expected = context.i64_type().const_array(&expected_words);
    let initializer = global.get_initializer();
    if global.get_value_type() != expected.get_type().as_any_type_enum()
        || global.get_linkage() != Linkage::External
        || global.get_unnamed_address() != UnnamedAddress::None
        || !global.is_constant()
        || global.get_alignment() != 8
        || initializer.map(|value| value.print_to_string()).as_ref()
            != Some(&expected.print_to_string())
    {
        return Err(CodegenError(format!(
            "predefined scan `{symbol}` does not match its canonical strong definition"
        )));
    }
    Ok(())
}

fn scan_words<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    scan: &RefScan,
) -> Result<Vec<inkwell::values::IntValue<'ctx>>, CodegenError> {
    let i64 = context.i64_type();
    Ok(match scan {
        RefScan::None => vec![i64.const_zero()],
        RefScan::References(offsets) => std::iter::once(i64.const_int(offsets.len() as u64, false))
            .chain(offsets.iter().map(|offset| i64.const_int(*offset, false)))
            .collect(),
        RefScan::Sequence(parts) => {
            let mut children = Vec::with_capacity(parts.len());
            for (index, part) in parts.iter().enumerate() {
                children.push(emit_private_scan(
                    context,
                    llvm,
                    &format!("{name}.part.{index}"),
                    part,
                )?);
            }
            std::iter::once(i64.const_int(SCAN_SEQUENCE, false))
                .chain(std::iter::once(i64.const_int(children.len() as u64, false)))
                .chain(children.into_iter().map(|child| child.const_to_int(i64)))
                .collect()
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let child = emit_private_scan(
                context,
                llvm,
                &format!("{name}.element"),
                element.as_ref_scan(),
            )?;
            vec![
                i64.const_int(SCAN_ARRAY, false),
                i64.const_int(*length_offset, false),
                i64.const_int(*first_element_offset, false),
                i64.const_int(stride.get(), false),
                child.const_to_int(i64),
            ]
        }
    })
}

fn emit_private_scan<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    name: &str,
    scan: &RefScan,
) -> Result<PointerValue<'ctx>, CodegenError> {
    let words = scan_words(context, llvm, name, scan)?;
    let value: BasicValueEnum<'ctx> = context.i64_type().const_array(&words).into();
    Ok(private_const_global(llvm, name, value))
}

fn require_definition(
    surface: &StrongObjectSymbolSurfaceV1,
    owner: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&scoop_lir::StrongDefinitionSymbolPlanV1, CodegenError> {
    let mut matches = surface
        .plans()
        .iter()
        .filter(|plan| plan.owner() == owner && plan.definition_role() == role);
    let definition = matches.next().ok_or_else(|| {
        CodegenError(format!(
            "canonical strong definition is missing owner {owner:?} role {role:?}"
        ))
    })?;
    if matches.next().is_some() {
        return Err(CodegenError(format!(
            "canonical strong definition duplicates owner {owner:?} role {role:?}"
        )));
    }
    Ok(definition)
}

fn require_absent_value(
    llvm: &LlvmModule<'_>,
    symbol: &str,
    role: &str,
) -> Result<(), CodegenError> {
    if llvm.get_global(symbol).is_some() || llvm.get_function(symbol).is_some() {
        Err(CodegenError(format!(
            "{role} `{symbol}` collides with an existing LLVM value"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn descriptor_diagnostic_atom(
    surface: &StrongObjectSymbolSurfaceV1,
    exact: scoop_lir::PersistentExactTypeId,
) -> Result<ObjectDefinitionAtomId, CodegenError> {
    let definition = descriptor_definition(surface, exact)?;
    let mut atoms = definition
        .atom_boundaries()
        .iter()
        .filter(|atom| atom.atom_role() == DefinitionAtomRole::AddressTakenConstant)
        .map(|atom| atom.atom());
    let atom = atoms.next().ok_or_else(|| {
        CodegenError(format!(
            "TypeDescriptor {exact} has no diagnostic associated atom"
        ))
    })?;
    if atoms.next().is_some() {
        return Err(CodegenError(format!(
            "TypeDescriptor {exact} has multiple diagnostic associated atoms"
        )));
    }
    Ok(atom)
}

pub(super) fn descriptor_itable_directory_atom(
    surface: &StrongObjectSymbolSurfaceV1,
    exact: scoop_lir::PersistentExactTypeId,
) -> Result<ObjectDefinitionAtomId, CodegenError> {
    let definition = descriptor_definition(surface, exact)?;
    let mut atoms = definition
        .atom_boundaries()
        .iter()
        .filter(|atom| atom.atom_role() == DefinitionAtomRole::RuntimeRecord)
        .map(|atom| atom.atom());
    let atom = atoms.next().ok_or_else(|| {
        CodegenError(format!(
            "TypeDescriptor {exact} has no itable-directory associated atom"
        ))
    })?;
    if atoms.next().is_some() {
        return Err(CodegenError(format!(
            "TypeDescriptor {exact} has multiple itable-directory associated atoms"
        )));
    }
    Ok(atom)
}

pub(super) fn descriptor_definition(
    surface: &StrongObjectSymbolSurfaceV1,
    exact: scoop_lir::PersistentExactTypeId,
) -> Result<&scoop_lir::StrongDefinitionSymbolPlanV1, CodegenError> {
    require_definition(
        surface,
        StrongDefinitionEntity::exact_type(exact),
        StrongDefinitionRole::TypeDescriptor,
    )
}
