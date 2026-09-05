use std::collections::{HashMap, HashSet};

use super::*;
use scoop_lir::{EnumDefId, GcEffect, StructDefId};

pub(crate) fn validate_module(module: &Module) -> Result<(), CodegenError> {
    validate_niche_representations(module)?;
    validate_machine_containers(module)?;
    validate_dispatch_callable_tables(module)?;
    validate_dispatch_signatures(module)?;
    validate_c_abi(module)?;
    validate_foreign_callbacks(module)
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
        let CallableRef::Local(id) = entry.callable else {
            if matches!(
                entry.callable,
                CallableRef::Runtime(scoop_lir::RuntimeFunction::Managed(
                    scoop_lir::ManagedRuntimeFunction::Alloc
                        | scoop_lir::ManagedRuntimeFunction::Box
                        | scoop_lir::ManagedRuntimeFunction::InitializationEnter
                ))
            ) {
                return Err(CodegenError(format!(
                    "type descriptor `{}` dispatches to a runtime function whose closed ABI contains an internal machine scalar",
                    descriptor.name
                )));
            }
            return Ok(());
        };
        let function = module
            .functions
            .get(id.into_u32() as usize)
            .ok_or_else(|| {
                CodegenError(format!(
                    "type descriptor `{}` has invalid local dispatch callable id {}",
                    descriptor.name,
                    id.into_u32()
                ))
            })?;
        if function
            .params
            .iter()
            .any(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
            || contains_machine_scalar(&module.structs, &module.enums, &function.return_ty)
        {
            return Err(CodegenError(format!(
                "type descriptor `{}` dispatches to local function @{} whose signature exposes an internal machine scalar",
                descriptor.name, function.symbol
            )));
        }
        Ok(())
    };

    for (_, descriptor) in module.meta.type_descriptors.iter() {
        for entry in &descriptor.vtable {
            validate_entry(descriptor, entry)?;
        }
        for record in &descriptor.itables {
            for entry in &record.slots {
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
                    function.symbol,
                    ty.dump()
                )));
            }
            Ok(())
        };
    for function in &module.functions {
        for (index, ty) in function.params.iter().enumerate() {
            validate_value_type(function, &format!("parameter {index}"), ty)?;
        }
        validate_value_type(function, "result", &function.return_ty)?;
        for (id, local) in function.locals.iter() {
            validate_value_type(
                function,
                &format!("local {}", id.into_raw().into_u32()),
                &local.ty,
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
        let GlobalInit::Storage { ty, .. } = &global.init else {
            continue;
        };
        if contains_machine_scalar(&module.structs, &module.enums, ty) {
            return Err(CodegenError(format!(
                "storage global `{}` has internal machine-scalar type {}",
                global.symbol,
                ty.dump()
            )));
        }
    }
    for (_, array) in module.meta.arrays.iter() {
        if array.element_align == 0 || !array.element_align.is_power_of_two() {
            return Err(CodegenError(format!(
                "array element layout has invalid size/alignment {}/{}",
                array.element_size, array.element_align
            )));
        }
        if contains_machine_scalar(&module.structs, &module.enums, &array.element) {
            return Err(CodegenError(format!(
                "array element type {} contains an internal machine scalar",
                array.element.dump()
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
                if signature
                    .params
                    .iter()
                    .any(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
                    || matches!(
                        &signature.return_type,
                        scoop_lir::LirReturnType::Value(ty)
                            if contains_machine_scalar(&module.structs, &module.enums, ty)
                    )
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
        params: &[LirType],
        result: Option<&LirType>,
    ) -> Result<(), CodegenError> {
        let has_machine = params
            .iter()
            .any(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
            || result.is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty));
        if has_machine {
            return Err(CodegenError(format!(
                "dispatch signature in @{} exposes an internal machine scalar without an authoritative dynamic-slot declaration",
                function.symbol
            )));
        }
        Ok(())
    }

    for function in &module.functions {
        let targets = &function.call_targets;
        for (_, target) in targets.managed_targets.void.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                check(
                    module,
                    function,
                    &targets.void_signatures[target.signature].params,
                    None,
                )?;
            }
        }
        for (_, target) in targets.managed_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(module, function, &signature.params, Some(&signature.result))?;
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
                    &signature.params,
                    Some(&signature.result.ty),
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
                    &targets.void_signatures[target.signature].params,
                    None,
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(module, function, &signature.params, Some(&signature.result))?;
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
                    &signature.params,
                    Some(&signature.result.ty),
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
        scoop_lir::CType::Integer(_) | scoop_lir::CType::Boolean => Ok(()),
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

fn foreign_callback_family<'a>(
    module: &'a Module,
    id: scoop_lir::ForeignCallbackFamilyId,
    owner: &str,
) -> Result<&'a scoop_lir::ForeignCallbackFamily, CodegenError> {
    if id.into_raw().into_u32() as usize >= module.foreign_callback_families.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid foreign callback family {}",
            id.into_raw()
        )));
    }
    Ok(&module.foreign_callback_families[id])
}

fn validate_foreign_callback_family(
    module: &Module,
    id: scoop_lir::ForeignCallbackFamilyId,
    family: &scoop_lir::ForeignCallbackFamily,
) -> Result<(), CodegenError> {
    let owner = format!("foreign callback family {}", id.into_raw());
    if family.callback.into_raw().into_u32() as usize >= module.structs.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid callback struct {}",
            family.callback.into_raw()
        )));
    }
    if family.state.into_raw().into_u32() as usize >= module.enums.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid state enum {}",
            family.state.into_raw()
        )));
    }
    if family.failure.into_raw().into_u32() as usize >= module.enums.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid failure enum {}",
            family.failure.into_raw()
        )));
    }

    let callback = &module.structs[family.callback];
    let callback_shape = callback.scoop_fields().is_some_and(|fields| {
        matches!(
            fields,
            [function, context]
                if function.ty == scoop_lir::CODE_PTR
                    && context.ty == scoop_lir::RAW_PTR
                    && function.layout.offset == 0
                    && context.layout.offset == 8
                    && function.layout.access_align == 8
                    && context.layout.access_align == 8
        )
    }) && !callback.interior_mutable
        && callback.size == 16
        && callback.align == 8;
    if !callback_shape {
        return Err(CodegenError(format!(
            "{owner} callback struct `{}` does not have the closed {{ ptr<code>, ptr<raw> }} representation",
            callback.name
        )));
    }

    let state = &module.enums[family.state];
    let state_shape = matches!(
        &state.repr,
        EnumRepr::Tagged {
            variants,
            size: 8,
            align: 8,
        } if variants.len() == 4 && variants.iter().all(|variant| variant.fields.is_empty())
    ) && !state.scan.contains_reference();
    if !state_shape {
        return Err(CodegenError(format!(
            "{owner} state enum `{}` does not have the closed four-state unit representation",
            state.name
        )));
    }

    let failure = &module.enums[family.failure];
    if !matches!(
        failure.repr,
        EnumRepr::Niche {
            kind: scoop_lir::NichePointerKind::Managed,
            ..
        }
    ) {
        return Err(CodegenError(format!(
            "{owner} failure enum `{}` is not a managed-reference niche enum",
            failure.name
        )));
    }
    Ok(())
}

fn validate_foreign_callbacks(module: &Module) -> Result<(), CodegenError> {
    let mut family_by_callback = HashMap::new();
    let mut protocol = None;
    for (id, family) in module.foreign_callback_families.iter() {
        validate_foreign_callback_family(module, id, family)?;
        if let Some(previous) = family_by_callback.insert(family.callback, id) {
            return Err(CodegenError(format!(
                "foreign callback struct {} belongs to both family {} and family {}",
                family.callback.into_raw(),
                previous.into_raw(),
                id.into_raw()
            )));
        }
        let identity = (family.state, family.failure);
        if let Some(expected) = protocol {
            if expected != identity {
                return Err(CodegenError(format!(
                    "foreign callback family {} conflicts with the module's nominal state/failure protocol",
                    id.into_raw()
                )));
            }
        } else {
            protocol = Some(identity);
        }
    }

    let expected_params = [
        scoop_lir::MANAGED_PTR,
        scoop_lir::RAW_PTR,
        scoop_lir::RAW_PTR,
        scoop_lir::RAW_PTR,
    ];
    let expected_result = LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus);
    let mut trampolines = HashMap::new();
    let mut signatures = HashMap::new();
    for (id, bridge) in module.foreign_callback_bridges.iter() {
        foreign_callback_family(
            module,
            bridge.family,
            &format!("foreign callback bridge {}", id.into_raw()),
        )?;
        let Some(context_type) = bridge.params.get(bridge.context_index as usize) else {
            return Err(CodegenError(format!(
                "foreign callback bridge {} context index {} is outside its {} C parameters",
                id.into_raw(),
                bridge.context_index,
                bridge.params.len()
            )));
        };
        if !matches!(
            context_type,
            scoop_lir::CType::DataPointer {
                pointee: scoop_lir::CDataPointee::OpaqueVoid,
                storage: scoop_lir::CDataPointerStorage::Direct,
            }
        ) {
            return Err(CodegenError(format!(
                "foreign callback bridge {} context parameter {} must be a direct opaque C data pointer, found {context_type:?}",
                id.into_raw(),
                bridge.context_index
            )));
        }

        let abi = (
            bridge.signature_symbol.as_str(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = trampolines.insert(bridge.trampoline_symbol.as_str(), abi) {
            if previous != abi {
                return Err(CodegenError(format!(
                    "foreign callback trampoline symbol @{} has conflicting ABI metadata",
                    bridge.trampoline_symbol
                )));
            }
        }
        let signature_abi = (
            bridge.trampoline_symbol.as_str(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = signatures.insert(bridge.signature_symbol.as_str(), signature_abi) {
            if previous != signature_abi {
                return Err(CodegenError(format!(
                    "foreign callback signature symbol @{} has conflicting ABI metadata",
                    bridge.signature_symbol
                )));
            }
        }

        let mut matches = module
            .functions
            .iter()
            .filter(|function| function.symbol == bridge.adapter_symbol);
        let Some(adapter) = matches.next() else {
            return Err(CodegenError(format!(
                "foreign callback adapter @{} is not a module function",
                bridge.adapter_symbol
            )));
        };
        if matches.next().is_some() {
            return Err(CodegenError(format!(
                "foreign callback adapter symbol @{} is not unique",
                bridge.adapter_symbol
            )));
        }
        if adapter.gc_effect != GcEffect::Managed
            || adapter.params.as_slice() != expected_params
            || adapter.return_ty != expected_result
        {
            return Err(CodegenError(format!(
                "foreign callback adapter @{} must be managed (ptr<managed>, ptr<raw>, ptr<raw>, ptr<raw>) -> machine<foreign-callback-status>",
                bridge.adapter_symbol
            )));
        }
    }

    for function in &module.functions {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::ForeignCallbackRegister {
                        out,
                        bridge,
                        closure,
                    } => {
                        if bridge.into_raw().into_u32() as usize
                            >= module.foreign_callback_bridges.len()
                        {
                            return Err(CodegenError(format!(
                                "foreign callback registration @{} references invalid bridge {}",
                                function.symbol,
                                bridge.into_raw()
                            )));
                        }
                        let bridge = &module.foreign_callback_bridges[*bridge];
                        let family = foreign_callback_family(
                            module,
                            bridge.family,
                            &format!("foreign callback registration @{}", function.symbol),
                        )?;
                        let closure_ty = function.value_ty(&module.globals, *closure);
                        let result_ty = &function.temps[*out].ty;
                        if closure_ty != scoop_lir::MANAGED_PTR
                            || result_ty != &LirType::Struct(family.callback)
                        {
                            return Err(CodegenError(format!(
                                "foreign callback registration @{} for family {} requires a managed closure and exact callback struct {}, got closure {} and result {}",
                                function.symbol,
                                bridge.family.into_raw(),
                                family.callback.into_raw(),
                                closure_ty.dump(),
                                result_ty.dump()
                            )));
                        }
                    }
                    Instruction::ForeignCallbackOperation(operation) => {
                        let family_id = operation.family();
                        let family = foreign_callback_family(
                            module,
                            family_id,
                            &format!("foreign callback operation @{}", function.symbol),
                        )?;
                        let callback_ty = function.value_ty(&module.globals, operation.callback());
                        if callback_ty != LirType::Struct(family.callback) {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} requires exact callback struct {}, got {}",
                                function.symbol,
                                family_id.into_raw(),
                                family.callback.into_raw(),
                                callback_ty.dump()
                            )));
                        }
                        let result_valid = match *operation {
                            scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                                function.temps[out].ty == LirType::Struct(family.callback)
                            }
                            scoop_lir::ForeignCallbackOperation::Release { .. } => true,
                            scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                                function.temps[out].ty == LirType::Enum(family.state)
                            }
                            scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                                function.temps[out].ty == LirType::Enum(family.failure)
                            }
                        };
                        if !result_valid {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} has a non-protocol result type",
                                function.symbol,
                                family_id.into_raw()
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
