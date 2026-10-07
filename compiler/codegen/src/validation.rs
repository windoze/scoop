use std::collections::{HashMap, HashSet};

use super::*;
use scoop_lir::{EnumDefId, GcEffect, StructDefId};

mod c_call_plan;
mod callbacks;
mod constants;
mod external;
use external::validate_external_metadata;
mod root_plans;
mod safepoints;
mod scoop_abi;
mod variants;
use callbacks::{validate_callback_declarations, validate_callback_instructions};
use constants::validate_constant_images;
use root_plans::validate_call_root_plans;
use safepoints::validate_safepoint_identities;
use scoop_abi::validate_scoop_abi;
use variants::validate_variant_primitives;

pub(crate) fn validate_module(module: &Module) -> Result<(), CodegenError> {
    validate_external_metadata(module)?;
    validate_type_descriptor_symbols(module)?;
    validate_array_metadata(module)?;
    validate_output(module)?;
    validate_niche_representations(module)?;
    validate_scoop_abi(module)?;
    validate_constant_images(module)?;
    validate_variant_primitives(module)?;
    validate_machine_containers(module)?;
    validate_dispatch_table_identities(module)?;
    validate_dispatch_callable_tables(module)?;
    validate_dispatch_signatures(module)?;
    validate_native_boundary(module)?;
    validate_callback_instructions(module)?;
    validate_safepoint_identities(module)?;
    validate_call_root_plans(module)?;
    scoop_lir::validate_startup_gateways(module)
        .map_err(|error| CodegenError(error.to_string()))?;
    Ok(())
}

pub(crate) fn validate_native_boundary(module: &Module) -> Result<(), CodegenError> {
    validate_c_abi(module)?;
    validate_callback_declarations(module)
}

fn validate_type_descriptor_symbols(module: &Module) -> Result<(), CodegenError> {
    match module.meta.well_known_type_descriptors.string {
        TypeDescriptorRef::Local(string) => {
            if arena_index(string) >= module.meta.type_descriptors.len() {
                return Err(CodegenError(format!(
                    "the runtime String TypeDescriptor local reference {} is out of bounds",
                    arena_index(string)
                )));
            }
        }
        TypeDescriptorRef::External(string) => {
            if arena_index(string) >= module.meta.external_type_descriptors.len() {
                return Err(CodegenError(format!(
                    "the runtime String TypeDescriptor external reference {} is out of bounds",
                    arena_index(string)
                )));
            }
        }
    }

    for (_, descriptor) in module.meta.type_descriptors.iter() {
        if descriptor.diagnostic_name.is_empty() {
            return Err(CodegenError(format!(
                "type descriptor {} has an empty canonical diagnostic name",
                descriptor.identity.exact_type()
            )));
        }
        if !descriptor
            .instance_layout
            .is_managed_instance_of(descriptor.identity.exact_type(), module.meta.target_profile)
        {
            return Err(CodegenError(format!(
                "type descriptor `{}` carries an instance layout for another exact type, target, representation, or scan role",
                descriptor.diagnostic_name
            )));
        }
    }
    Ok(())
}

fn validate_array_metadata(module: &Module) -> Result<(), CodegenError> {
    for (_, array) in module.meta.arrays.iter() {
        if contains_machine_scalar(&module.structs, &module.enums, &array.element) {
            return Err(CodegenError(format!(
                "array element type {} contains an internal machine scalar",
                array.element.dump()
            )));
        }
        let TypeDescriptorRef::Local(descriptor_id) = array.type_descriptor else {
            return Err(CodegenError(
                "array metadata requires a complete local TypeDescriptor".to_string(),
            ));
        };
        let descriptor_index = arena_index(descriptor_id);
        if descriptor_index >= module.meta.type_descriptors.len() {
            return Err(CodegenError(format!(
                "array metadata has invalid local TypeDescriptor id {descriptor_index}"
            )));
        }
        let descriptor = &module.meta.type_descriptors[descriptor_id];
        let exact = descriptor.identity.exact_type();
        if !array
            .identity
            .is_managed_array_of(exact, module.meta.target_profile)
        {
            return Err(CodegenError(format!(
                "array TypeDescriptor `{}` and its element layout identify different exact types, targets, representations, or scan roles",
                descriptor.diagnostic_name
            )));
        }
        let expected = array.layout.instance();
        if &descriptor.instance_shape != expected {
            return Err(CodegenError(format!(
                "array TypeDescriptor `{}` does not match its closed element size, alignment, and scan shape",
                descriptor.diagnostic_name
            )));
        }
    }
    for function in module.callable_bodies() {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                let Instruction::ArrayClone {
                    source_type,
                    array_type,
                    ..
                } = instruction
                else {
                    continue;
                };
                if arena_index(*source_type) >= module.meta.arrays.len()
                    || arena_index(*array_type) >= module.meta.arrays.len()
                {
                    return Err(CodegenError(
                        "array clone source or target metadata is missing".into(),
                    ));
                }
                let source = &module.meta.arrays[*source_type];
                let target = &module.meta.arrays[*array_type];
                if source.element_exact != target.element_exact
                    || source.element != target.element
                    || source.layout != target.layout
                {
                    return Err(CodegenError(
                        "array clone exact element type or storage disagrees".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn executable_entry(
    module: &Module,
    entry: scoop_lir::LocalFunctionRef,
) -> Result<&Function, CodegenError> {
    let (declaration, expected_effect) = match entry {
        scoop_lir::LocalFunctionRef::Managed(reference) => {
            (reference.declaration(), GcEffect::Managed)
        }
        scoop_lir::LocalFunctionRef::NoGc(reference) => (reference.declaration(), GcEffect::NoGc),
    };
    let index = declaration.into_u32() as usize;
    let function = module.functions.get(index).ok_or_else(|| {
        CodegenError(format!(
            "module has invalid executable entry function id {index}"
        ))
    })?;
    if function.gc_effect != expected_effect {
        return Err(CodegenError(format!(
            "executable entry @{} has {:?} body but {:?} typed reference",
            function.symbol(),
            function.gc_effect,
            expected_effect
        )));
    }
    Ok(function)
}

fn validate_output(module: &Module) -> Result<(), CodegenError> {
    let scoop_lir::LirOutput::Executable { entry } = module.output else {
        return Ok(());
    };
    executable_entry(module, entry).map(|_| ())
}

pub(crate) fn validate_executable_entry(
    module: &Module,
    entry: scoop_lir::LocalFunctionRef,
) -> Result<(), CodegenError> {
    let function = executable_entry(module, entry)?;
    if !function.signature.arguments().is_empty()
        || !matches!(function.signature.result(), scoop_lir::AbiReturn::UnitVoid)
    {
        return Err(CodegenError(format!(
            "executable entry @{} must have signature () -> Unit",
            function.symbol()
        )));
    }
    Ok(())
}

fn validate_dispatch_table_identities(module: &Module) -> Result<(), CodegenError> {
    for (_, descriptor) in module.meta.type_descriptors.iter() {
        let owner = descriptor.identity.exact_type();
        if !descriptor.vtable.belongs_to_exact_type(owner) {
            return Err(CodegenError(format!(
                "type descriptor `{}` carries a vtable identity for another exact type or table role",
                descriptor.diagnostic_name
            )));
        }

        for itable in &descriptor.itables {
            if !itable.belongs_to_exact_type(owner) {
                return Err(CodegenError(format!(
                    "type descriptor `{}` carries an itable identity for another exact type or table role",
                    descriptor.diagnostic_name
                )));
            }
            let TypeDescriptorRef::Local(interface_id) = itable.interface() else {
                continue;
            };
            let interface_index = arena_index(interface_id);
            if interface_index >= module.meta.type_descriptors.len() {
                return Err(CodegenError(format!(
                    "type descriptor `{}` has invalid local itable interface id {interface_index}",
                    descriptor.diagnostic_name
                )));
            }
            let interface = &module.meta.type_descriptors[interface_id];
            if !itable.belongs_to_interface_exact_type(interface.identity.exact_type()) {
                return Err(CodegenError(format!(
                    "type descriptor `{}` itable identity and interface descriptor identify different exact types",
                    descriptor.diagnostic_name
                )));
            }
        }
    }
    Ok(())
}

fn validate_variant_ref(
    module: &Module,
    variant: scoop_lir::LirVariantRef,
    owner: &str,
) -> Result<(), CodegenError> {
    if !module.enums.contains_variant(variant) {
        return Err(CodegenError(format!(
            "{owner} carries invalid enum{} variant {} reference",
            variant.definition().into_raw(),
            variant.index()
        )));
    }
    Ok(())
}

fn checked_temp_type<'a>(
    function: &'a Function,
    temp: scoop_lir::TempId,
    owner: &str,
) -> Result<&'a LirType, CodegenError> {
    let index = arena_index(temp);
    if index >= function.temps.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid temporary t{index} in @{}",
            function.symbol()
        )));
    }
    Ok(&function.temps[temp].ty)
}

fn checked_value_type(
    module: &Module,
    function: &Function,
    value: Value,
    owner: &str,
) -> Result<LirType, CodegenError> {
    let invalid = |kind: &str, index: usize| {
        CodegenError(format!(
            "{owner} references invalid {kind} {index} in @{}",
            function.symbol()
        ))
    };
    match value {
        Value::Local(id) => {
            let index = arena_index(id);
            if index >= function.locals.len() {
                return Err(invalid("local", index));
            }
            Ok(function.locals[id].ty().clone())
        }
        Value::Param(index) => function
            .signature
            .arguments()
            .get(index as usize)
            .map(|argument| argument.logical_storage_type().clone())
            .ok_or_else(|| invalid("parameter", index as usize)),
        Value::Temp(id) => checked_temp_type(function, id, owner).cloned(),
        Value::IntegerConst(value) => Ok(value.scalar_type()),
        Value::FloatConst(value) => Ok(LirType::floating(value.kind())),
        Value::MachineScalar(value) => Ok(LirType::MachineScalar(value.kind())),
        Value::BoolConst(_) => Ok(LirType::I1),
        Value::ContextKeyCell(_) => Ok(scoop_lir::RAW_PTR),
        Value::NullPointer(kind) => Ok(LirType::Ptr(kind)),
        Value::TypeDescriptor(reference) => {
            let (kind, index, len) = match reference {
                scoop_lir::TypeDescriptorRef::Local(id) => (
                    "local type descriptor",
                    arena_index(id),
                    module.meta.type_descriptors.len(),
                ),
                scoop_lir::TypeDescriptorRef::External(id) => (
                    "external type descriptor",
                    arena_index(id),
                    module.meta.external_type_descriptors.len(),
                ),
            };
            if index >= len {
                return Err(invalid(kind, index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::RootScan(id) => {
            let index = arena_index(id);
            if index >= function.call_targets.root_scans.len() {
                return Err(invalid("root scan", index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::Global(id) => {
            let index = arena_index(id);
            if index >= module.globals.len() {
                return Err(invalid("global", index));
            }
            Ok(LirType::Ptr(module.globals[id].address_kind))
        }
        Value::InitializationUnit(id) => {
            let index = arena_index(id);
            if index >= module.initialization_units.len() {
                return Err(invalid("initialization unit", index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::CArgumentStorage(storage) => {
            let index = arena_index(storage.local());
            if index >= function.locals.len() {
                return Err(invalid("C argument local", index));
            }
            Ok(scoop_lir::RAW_PTR)
        }
    }
}

fn validate_niche_representations(module: &Module) -> Result<(), CodegenError> {
    for (_, definition) in module.enums.iter() {
        let EnumRepr::Niche {
            kind,
            payload_variant,
        } = &definition.repr
        else {
            continue;
        };
        if *payload_variant > 1 {
            return Err(CodegenError(format!(
                "niche enum `{}` has invalid payload variant {payload_variant}",
                definition.name
            )));
        }
        let valid_scan = match kind {
            scoop_lir::NichePointerKind::Managed => definition.scan == RefScan::References(vec![0]),
            scoop_lir::NichePointerKind::Raw | scoop_lir::NichePointerKind::Code => {
                definition.scan == RefScan::None
            }
        };
        if !valid_scan {
            return Err(CodegenError(format!(
                "niche enum `{}` has {} pointer provenance but incompatible scan {}",
                definition.name,
                kind.pointer_kind().dump(),
                definition.scan.dump(),
            )));
        }
    }
    Ok(())
}

fn validate_dispatch_callable_tables(module: &Module) -> Result<(), CodegenError> {
    let validate_entry = |descriptor: &TypeDescriptor,
                          entry: &DispatchEntry|
     -> Result<(), CodegenError> {
        let id = match entry.callable {
            CallableRef::Local(id) => id,
            CallableRef::External(id) => {
                if arena_index(id) >= module.meta.external_callables.len() {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` has invalid external dispatch callable id {}",
                        descriptor.diagnostic_name,
                        arena_index(id)
                    )));
                }
                return Ok(());
            }
            CallableRef::Runtime(runtime) => {
                if runtime.requires_dedicated_operation() {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` dispatches to a dedicated boxing operation",
                        descriptor.diagnostic_name
                    )));
                }
                if matches!(
                    runtime,
                    scoop_lir::RuntimeFunction::Managed(
                        scoop_lir::ManagedRuntimeFunction::Alloc
                            | scoop_lir::ManagedRuntimeFunction::InitializationEnter
                    )
                ) {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` dispatches to a runtime function whose closed ABI contains an internal machine scalar",
                        descriptor.diagnostic_name
                    )));
                }
                return Ok(());
            }
        };
        let function = module
            .functions
            .get(id.into_u32() as usize)
            .ok_or_else(|| {
                CodegenError(format!(
                    "type descriptor `{}` has invalid local dispatch callable id {}",
                    descriptor.diagnostic_name,
                    id.into_u32()
                ))
            })?;
        if function.signature.arguments().iter().any(|argument| {
            contains_machine_scalar(
                &module.structs,
                &module.enums,
                argument.logical_storage_type(),
            )
        }) || function
            .signature
            .result()
            .logical_storage_type()
            .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
        {
            return Err(CodegenError(format!(
                "type descriptor `{}` dispatches to local function @{} whose signature exposes an internal machine scalar",
                descriptor.diagnostic_name,
                function.symbol()
            )));
        }
        Ok(())
    };

    for (_, descriptor) in module.meta.type_descriptors.iter() {
        for entry in descriptor.vtable.slots() {
            validate_entry(descriptor, entry)?;
        }
        for record in &descriptor.itables {
            for entry in record.slots() {
                validate_entry(descriptor, entry)?;
            }
        }
    }
    Ok(())
}

fn validate_machine_containers(module: &Module) -> Result<(), CodegenError> {
    for (_, definition) in module.structs.iter() {
        for index in 0..definition.field_count() {
            let field_type = definition
                .field_storage_type(index)
                .expect("index is below the field count");
            if contains_machine_scalar(&module.structs, &module.enums, &field_type) {
                return Err(CodegenError(format!(
                    "struct `{}` field {} embeds an internal machine scalar in a source value container",
                    definition.name, index
                )));
            }
        }
    }
    let validate_value_type =
        |function: &Function, owner: &str, ty: &LirType| -> Result<(), CodegenError> {
            if !matches!(ty, LirType::MachineScalar(_))
                && contains_machine_scalar(&module.structs, &module.enums, ty)
            {
                return Err(CodegenError(format!(
                    "function @{} {owner} embeds an internal machine scalar in {}",
                    function.symbol(),
                    ty.dump()
                )));
            }
            Ok(())
        };
    for function in module.callable_bodies() {
        for (index, argument) in function.signature.arguments().iter().enumerate() {
            validate_value_type(
                function,
                &format!("parameter {index}"),
                argument.logical_storage_type(),
            )?;
        }
        if let Some(result) = function.signature.result().logical_storage_type() {
            validate_value_type(function, "result", result)?;
        }
        for (id, local) in function.locals.iter() {
            validate_value_type(
                function,
                &format!("local {}", id.into_raw().into_u32()),
                local.ty(),
            )?;
        }
        for (id, temp) in function.temps.iter() {
            validate_value_type(
                function,
                &format!("temporary {}", id.into_raw().into_u32()),
                &temp.ty,
            )?;
        }
    }
    for (_, global) in module.globals.iter() {
        let (GlobalInit::Storage { ty, .. }
        | GlobalInit::RawStorage { ty, .. }
        | GlobalInit::ImportedStorage { ty, .. }) = &global.init
        else {
            continue;
        };
        if contains_machine_scalar(&module.structs, &module.enums, ty) {
            return Err(CodegenError(format!(
                "storage global `{}` has internal machine-scalar type {}",
                global.symbol(),
                ty.dump()
            )));
        }
    }
    for (_, definition) in module.enums.iter() {
        let EnumRepr::Tagged { variants, .. } = &definition.repr else {
            continue;
        };
        for variant in variants {
            for field in &variant.fields {
                if contains_machine_scalar(&module.structs, &module.enums, &field.ty) {
                    return Err(CodegenError(format!(
                        "enum `{}` payload type {} contains an internal machine scalar",
                        definition.name,
                        field.ty.dump()
                    )));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn contains_machine_scalar(
    structs: &StructDefs,
    enums: &EnumDefs,
    ty: &LirType,
) -> bool {
    fn visit(
        structs: &StructDefs,
        enums: &EnumDefs,
        ty: &LirType,
        seen_structs: &mut HashSet<StructDefId>,
        seen_enums: &mut HashSet<EnumDefId>,
    ) -> bool {
        match ty {
            LirType::MachineScalar(_) => true,
            LirType::Aggregate(elements) => elements
                .iter()
                .any(|element| visit(structs, enums, element, seen_structs, seen_enums)),
            LirType::Struct(id) if seen_structs.insert(*id) => {
                let definition = &structs[*id];
                (0..definition.field_count()).any(|index| {
                    let field_type = definition
                        .field_storage_type(index)
                        .expect("index is below the field count");
                    visit(structs, enums, &field_type, seen_structs, seen_enums)
                })
            }
            LirType::Enum(id) if seen_enums.insert(*id) => match &enums[*id].repr {
                EnumRepr::Niche { .. } => false,
                EnumRepr::Tagged { variants, .. } => variants.iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| visit(structs, enums, &field.ty, seen_structs, seen_enums))
                }),
            },
            _ => false,
        }
    }

    visit(structs, enums, ty, &mut HashSet::new(), &mut HashSet::new())
}

fn validate_c_abi(module: &Module) -> Result<(), CodegenError> {
    for (_, function) in module.extern_functions.iter() {
        match &function.kind {
            ExternFunctionKind::C { signature, .. } => {
                c_call_plan::validate(function)?;
                for (index, parameter) in signature.params.iter().enumerate() {
                    validate_c_type(module, parameter, false, &mut HashSet::new()).map_err(
                        |error| {
                            CodegenError(format!(
                                "C extern `{}` parameter {}: {}",
                                function.source_name, index, error.0
                            ))
                        },
                    )?;
                }
                validate_c_return_type(module, &signature.return_type).map_err(|error| {
                    CodegenError(format!(
                        "C extern `{}` result: {}",
                        function.source_name, error.0
                    ))
                })?;
            }
            ExternFunctionKind::Scoop { signature, .. } => {
                if signature.arguments().iter().any(|argument| {
                    contains_machine_scalar(
                        &module.structs,
                        &module.enums,
                        argument.logical_storage_type(),
                    )
                }) || signature
                    .result()
                    .logical_storage_type()
                    .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
                {
                    return Err(CodegenError(format!(
                        "Scoop extern `{}` exposes an internal machine scalar across an artifact boundary",
                        function.source_name
                    )));
                }
            }
        }
    }

    for (_, global) in module.native_globals.iter() {
        validate_c_type(module, &global.c_type, false, &mut HashSet::new()).map_err(|error| {
            CodegenError(format!(
                "native global `{}`: {}",
                global.source_name, error.0
            ))
        })?;
    }
    for (_, callback) in module.callback_bridges.iter() {
        let effect = match callback.bridge {
            scoop_lir::StaticCallbackTarget::Local(bridge) => module
                .functions
                .get(bridge.declaration().into_u32() as usize)
                .map(|function| function.gc_effect),
            scoop_lir::StaticCallbackTarget::External(bridge) => module
                .meta
                .external_callables
                .iter()
                .find_map(|(id, function)| (id == bridge).then_some(function.gc_effect())),
        };
        if effect != Some(GcEffect::NoGc) {
            return Err(CodegenError(format!(
                "static callback `{}` must reference an existing NoGC storage bridge",
                callback.source_name
            )));
        }
        for parameter in &callback.params {
            validate_c_type(module, parameter, false, &mut HashSet::new())?;
        }
        validate_c_return_type(module, &callback.return_type)?;
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        for parameter in &callback.params {
            validate_c_type(module, parameter, false, &mut HashSet::new())?;
        }
        validate_c_return_type(module, &callback.return_type)?;
    }
    validate_c_layouts(module)?;
    Ok(())
}

pub(crate) fn validate_c_layouts(module: &Module) -> Result<(), CodegenError> {
    for (id, definition) in module.structs.iter() {
        if definition.is_c_layout() {
            validate_c_struct(module, id, &mut HashSet::new())?;
        }
    }
    Ok(())
}

fn validate_dispatch_signatures(module: &Module) -> Result<(), CodegenError> {
    fn check(
        module: &Module,
        function: &Function,
        params: &[scoop_lir::AbiArgument],
        result: Option<&LirType>,
    ) -> Result<(), CodegenError> {
        let has_machine = params.iter().any(|argument| {
            contains_machine_scalar(
                &module.structs,
                &module.enums,
                argument.logical_storage_type(),
            )
        }) || result
            .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty));
        if has_machine {
            return Err(CodegenError(format!(
                "dispatch signature in @{} exposes an internal machine scalar without an authoritative dynamic-slot declaration",
                function.symbol()
            )));
        }
        Ok(())
    }

    for function in module.callable_bodies() {
        let targets = &function.call_targets;
        for (_, target) in targets.managed_targets.void.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                check(
                    module,
                    function,
                    targets.void_signatures[target.signature].arguments(),
                    None,
                )?;
            }
        }
        for (_, target) in targets.managed_targets.elided_zst.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.elided_zst_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.managed_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.managed_targets.indirect_result.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.indirect_result_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.void.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                check(
                    module,
                    function,
                    targets.void_signatures[target.signature].arguments(),
                    None,
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.elided_zst.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.elided_zst_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.indirect_result.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.indirect_result_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_c_return_type(
    module: &Module,
    ty: &scoop_lir::CReturnType,
) -> Result<(), CodegenError> {
    match ty {
        scoop_lir::CReturnType::Void => Ok(()),
        scoop_lir::CReturnType::Value(ty) => {
            validate_c_type(module, ty, false, &mut HashSet::new())
        }
    }
}

fn niche_pointer_kind(module: &Module, lir: &LirType) -> Option<scoop_lir::NichePointerKind> {
    let LirType::Enum(id) = lir else {
        return None;
    };
    match &module.enums[*id].repr {
        EnumRepr::Niche { kind, .. } => Some(*kind),
        EnumRepr::Tagged { .. } => None,
    }
}

fn validate_c_struct(
    module: &Module,
    id: StructDefId,
    visiting: &mut HashSet<StructDefId>,
) -> Result<(), CodegenError> {
    let definition = &module.structs[id];
    let Some(fields) = definition.c_fields() else {
        return Err(CodegenError(format!(
            "struct `{}` is not a C-layout struct",
            definition.name
        )));
    };
    if !visiting.insert(id) {
        return Err(CodegenError(format!(
            "C-layout struct `{}` is recursively embedded by value",
            definition.name
        )));
    }
    for (index, field) in fields.iter().enumerate() {
        validate_c_type(module, &field.ty, true, visiting).map_err(|error| {
            CodegenError(format!(
                "C-layout struct `{}` field {}: {}",
                definition.name, index, error.0
            ))
        })?;
    }
    visiting.remove(&id);
    Ok(())
}

fn validate_c_type(
    module: &Module,
    ty: &scoop_lir::CType,
    struct_by_value: bool,
    visiting: &mut HashSet<StructDefId>,
) -> Result<(), CodegenError> {
    match ty {
        scoop_lir::CType::Float(_) | scoop_lir::CType::Integer(_) | scoop_lir::CType::Boolean => {
            Ok(())
        }
        scoop_lir::CType::DataPointer { pointee, storage } => {
            if let scoop_lir::CDataPointerStorage::Nullable(reference) = storage {
                let id = reference.definition();
                let Some(binding) = module.enums.nullable_data_pointer_binding(id) else {
                    return Err(CodegenError(format!(
                        "nullable C data pointer references enum {} without an exact data-pointer binding owned by this module",
                        id.into_raw()
                    )));
                };
                if binding != reference.pointee() {
                    return Err(CodegenError(format!(
                        "nullable C data pointer enum `{}` reference disagrees with its owned exact pointee binding",
                        module.enums[id].name
                    )));
                }
                if pointee != reference.pointee() {
                    return Err(CodegenError(format!(
                        "nullable C data pointer enum `{}` binds a different exact pointee",
                        module.enums[id].name
                    )));
                }
                if niche_pointer_kind(module, &LirType::Enum(id))
                    != Some(scoop_lir::NichePointerKind::Raw)
                {
                    return Err(CodegenError(format!(
                        "nullable C data pointer references enum `{}` without raw-pointer provenance",
                        module.enums[id].name
                    )));
                }
            }
            if let scoop_lir::CDataPointee::Object(pointee) = pointee {
                validate_c_type(module, pointee, false, visiting)?;
            }
            Ok(())
        }
        scoop_lir::CType::CodePointer { signature, storage } => {
            if let scoop_lir::CCodePointerStorage::Nullable(reference) = storage {
                let id = reference.definition();
                let Some(binding) = module.enums.nullable_code_pointer_binding(id) else {
                    return Err(CodegenError(format!(
                        "nullable C code pointer references enum {} without an exact code-pointer binding owned by this module",
                        id.into_raw()
                    )));
                };
                if binding != reference.signature() {
                    return Err(CodegenError(format!(
                        "nullable C code pointer enum `{}` reference disagrees with its owned exact signature binding",
                        module.enums[id].name
                    )));
                }
                if signature.as_ref() != reference.signature() {
                    return Err(CodegenError(format!(
                        "nullable C code pointer enum `{}` binds a different exact signature",
                        module.enums[id].name
                    )));
                }
                if niche_pointer_kind(module, &LirType::Enum(id))
                    != Some(scoop_lir::NichePointerKind::Code)
                {
                    return Err(CodegenError(format!(
                        "nullable C code pointer references enum `{}` without code-pointer provenance",
                        module.enums[id].name
                    )));
                }
            }
            for parameter in &signature.params {
                validate_c_type(module, parameter, false, visiting)?;
            }
            if let scoop_lir::CReturnType::Value(result) = &signature.return_type {
                validate_c_type(module, result, false, visiting)?;
            }
            Ok(())
        }
        scoop_lir::CType::Struct(reference) => {
            let id = reference.definition();
            if struct_by_value {
                validate_c_struct(module, id, visiting)
            } else if module.structs[id].is_c_layout() {
                Ok(())
            } else {
                Err(CodegenError(format!(
                    "struct `{}` is not a C-layout struct",
                    module.structs[id].name
                )))
            }
        }
    }
}
