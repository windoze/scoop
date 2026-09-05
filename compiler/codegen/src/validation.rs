use std::collections::{HashMap, HashSet};

use super::*;
use scoop_lir::{EnumDefId, GcEffect, StructDefId};

pub(crate) fn validate_module(module: &Module) -> Result<(), CodegenError> {
    validate_machine_containers(module)?;
    validate_dispatch_callable_tables(module)?;
    validate_dispatch_signatures(module)?;
    validate_c_abi(module)?;
    validate_foreign_callbacks(module)
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
        for (index, field) in definition.fields.iter().enumerate() {
            if contains_machine_scalar(&module.structs, &module.enums, &field.ty) {
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
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    ty: &LirType,
) -> bool {
    fn visit(
        structs: &Arena<StructDef>,
        enums: &Arena<EnumDef>,
        ty: &LirType,
        seen_structs: &mut HashSet<StructDefId>,
        seen_enums: &mut HashSet<EnumDefId>,
    ) -> bool {
        match ty {
            LirType::MachineScalar(_) => true,
            LirType::Aggregate(elements) => elements
                .iter()
                .any(|element| visit(structs, enums, element, seen_structs, seen_enums)),
            LirType::Struct(id) if seen_structs.insert(*id) => structs[*id]
                .fields
                .iter()
                .any(|field| visit(structs, enums, &field.ty, seen_structs, seen_enums)),
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
        let ExternFunctionKind::C {
            params,
            return_type,
            ..
        } = &function.kind
        else {
            if function
                .params
                .iter()
                .any(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
                || contains_machine_scalar(&module.structs, &module.enums, &function.return_type)
            {
                return Err(CodegenError(format!(
                    "Scoop extern `{}` exposes an internal machine scalar across an artifact boundary",
                    function.source_name
                )));
            }
            continue;
        };
        if function.params.len() != params.len() {
            return Err(CodegenError(format!(
                "C extern `{}` has {} LIR parameters but {} C parameters",
                function.source_name,
                function.params.len(),
                params.len()
            )));
        }
        for (index, (lir, c)) in function.params.iter().zip(params).enumerate() {
            validate_c_pair(module, lir, c, false).map_err(|error| {
                CodegenError(format!(
                    "C extern `{}` parameter {}: {}",
                    function.source_name, index, error.0
                ))
            })?;
        }
        validate_c_pair(module, &function.return_type, return_type, true).map_err(|error| {
            CodegenError(format!(
                "C extern `{}` result: {}",
                function.source_name, error.0
            ))
        })?;
    }

    for (_, global) in module.native_globals.iter() {
        validate_c_pair(module, &global.ty, &global.c_type, false).map_err(|error| {
            CodegenError(format!(
                "native global `{}`: {}",
                global.source_name, error.0
            ))
        })?;
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

fn validate_c_pair(
    module: &Module,
    lir: &LirType,
    c: &scoop_lir::CType,
    allow_unit: bool,
) -> Result<(), CodegenError> {
    let matches = match c {
        scoop_lir::CType::Unit => {
            allow_unit
                && (matches!(lir, LirType::Void)
                    || matches!(lir, LirType::Aggregate(elements) if elements.is_empty()))
        }
        scoop_lir::CType::Int | scoop_lir::CType::UInt => matches!(lir, LirType::I64),
        scoop_lir::CType::Boolean => matches!(lir, LirType::I1),
        scoop_lir::CType::Pointer => is_c_pointer(module, lir),
        scoop_lir::CType::FunctionPointer { .. } => {
            matches!(lir, LirType::Ptr(PointerKind::Code)) || is_c_pointer_enum(module, lir)
        }
        scoop_lir::CType::Struct(c_id) => match lir {
            LirType::Struct(lir_id) if lir_id == c_id => {
                validate_c_struct(module, *lir_id, &mut HashSet::new())?;
                true
            }
            _ => false,
        },
    };
    if matches {
        Ok(())
    } else {
        Err(CodegenError(format!(
            "LIR type {} does not exactly match C type {c:?}",
            lir.dump()
        )))
    }
}

fn is_c_pointer(module: &Module, lir: &LirType) -> bool {
    matches!(lir, LirType::Ptr(PointerKind::Raw)) || is_c_pointer_enum(module, lir)
}

fn is_c_pointer_enum(module: &Module, lir: &LirType) -> bool {
    let LirType::Enum(id) = lir else {
        return false;
    };
    matches!(module.enums[*id].repr, EnumRepr::Niche { .. })
        && !module.enums[*id].scan.contains_reference()
}

fn validate_c_struct(
    module: &Module,
    id: StructDefId,
    visiting: &mut HashSet<StructDefId>,
) -> Result<(), CodegenError> {
    let definition = &module.structs[id];
    if definition.c_layout.is_none() {
        return Err(CodegenError(format!(
            "struct `{}` is not a C-layout struct",
            definition.name
        )));
    }
    if !visiting.insert(id) {
        return Err(CodegenError(format!(
            "C-layout struct `{}` is recursively embedded by value",
            definition.name
        )));
    }
    for (index, field) in definition.fields.iter().enumerate() {
        let valid = match &field.ty {
            LirType::I1 | LirType::I64 => true,
            LirType::Ptr(PointerKind::Raw | PointerKind::Code) => true,
            LirType::Struct(nested) => {
                validate_c_struct(module, *nested, visiting)?;
                true
            }
            ty if is_c_pointer_enum(module, ty) => true,
            _ => false,
        };
        if !valid {
            return Err(CodegenError(format!(
                "C-layout struct `{}` field {} has non-C type {}",
                definition.name,
                index,
                field.ty.dump()
            )));
        }
    }
    visiting.remove(&id);
    Ok(())
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
    let callback_shape = matches!(
        callback.fields.as_slice(),
        [function, context]
            if function.ty == scoop_lir::CODE_PTR
                && context.ty == scoop_lir::RAW_PTR
                && function.layout.offset == 0
                && context.layout.offset == 8
                && function.layout.access_align == 8
                && context.layout.access_align == 8
    ) && callback.c_layout.is_none()
        && !callback.interior_mutable
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
    if !matches!(failure.repr, EnumRepr::Niche { .. }) || !failure.scan.contains_reference() {
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
        if context_type != &scoop_lir::CType::Pointer {
            return Err(CodegenError(format!(
                "foreign callback bridge {} context parameter {} must be CType::Pointer, found {context_type:?}",
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
