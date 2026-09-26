use super::*;

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

    let modes = scoop_lir::ForeignCallbackModes::checked(
        &module.enums,
        family.modes.reusable(),
        family.modes.one_shot(),
    );
    if modes != Some(family.modes) {
        return Err(CodegenError(format!(
            "{owner} carries invalid callback mode variant metadata"
        )));
    }
    let states = scoop_lir::ForeignCallbackStates::checked(
        &module.enums,
        family.states.registered(),
        family.states.active(),
        family.states.completed(),
        family.states.failed(),
    );
    if states != Some(family.states) {
        return Err(CodegenError(format!(
            "{owner} carries invalid callback state variant metadata"
        )));
    }
    let state = &module.enums[family.states.definition()];
    if !matches!(
        &state.repr,
        EnumRepr::Tagged {
            size: 8,
            align: 8,
            ..
        }
    ) || state.scan.contains_reference()
    {
        return Err(CodegenError(format!(
            "{owner} state enum `{}` does not have the closed four-state unit representation",
            state.name
        )));
    }
    let failure_result = scoop_lir::ForeignCallbackFailureResult::checked(
        &module.enums,
        family.failure_result.some_payload(),
        family.failure_result.none(),
    );
    if failure_result != Some(family.failure_result) {
        return Err(CodegenError(format!(
            "{owner} carries invalid managed-reference callback failure metadata"
        )));
    }
    Ok(())
}

pub(super) fn validate_callback_declarations(module: &Module) -> Result<(), CodegenError> {
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
        let identity = (family.modes, family.states, family.failure_result);
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
        let family = &module.foreign_callback_families[bridge.family];
        if family.modes.runtime_code(bridge.mode).is_none() {
            return Err(CodegenError(format!(
                "foreign callback bridge {} carries a mode outside family {}",
                id.into_raw(),
                bridge.family.into_raw()
            )));
        }
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
            bridge.trampoline.signature_descriptor_symbol(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = trampolines.insert(bridge.trampoline.entry().symbol(), abi) {
            if previous != abi {
                return Err(CodegenError(format!(
                    "foreign callback trampoline symbol @{} has conflicting ABI metadata",
                    bridge.trampoline.entry().symbol()
                )));
            }
        }
        let signature_abi = (
            bridge.trampoline.entry().symbol(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = signatures.insert(
            bridge.trampoline.signature_descriptor_symbol(),
            signature_abi,
        ) {
            if previous != signature_abi {
                return Err(CodegenError(format!(
                    "foreign callback signature symbol @{} has conflicting ABI metadata",
                    bridge.trampoline.signature_descriptor_symbol()
                )));
            }
        }

        let adapter_index = bridge.adapter.declaration().into_u32() as usize;
        let Some(adapter) = module.functions.get(adapter_index) else {
            return Err(CodegenError(format!(
                "foreign callback bridge {} has invalid managed adapter id {adapter_index}",
                id.into_raw()
            )));
        };
        let adapter_params_match = adapter.signature.arguments().len() == expected_params.len()
            && adapter
                .signature
                .arguments()
                .iter()
                .zip(expected_params.iter())
                .all(|(argument, expected)| {
                    matches!(argument, scoop_lir::AbiArgument::Direct(value)
                        if value.storage_type() == expected)
                });
        let adapter_result_matches = matches!(
            adapter.signature.result(),
            scoop_lir::AbiReturn::Direct(value) if value.storage_type() == &expected_result
        );
        if adapter.gc_effect != GcEffect::Managed
            || !adapter_params_match
            || !adapter_result_matches
        {
            return Err(CodegenError(format!(
                "foreign callback adapter @{} must be managed (ptr<managed>, ptr<raw>, ptr<raw>, ptr<raw>) -> machine<foreign-callback-status>",
                adapter.symbol()
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_callback_instructions(module: &Module) -> Result<(), CodegenError> {
    for function in &module.functions {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::FunctionAddress { out, target } => {
                        if checked_temp_type(function, *out, "function address result")?
                            != &scoop_lir::CODE_PTR
                        {
                            return Err(CodegenError(format!(
                                "function address @{} must produce ptr<code>",
                                function.symbol()
                            )));
                        }
                        match target {
                            scoop_lir::FunctionAddressTarget::Local(reference) => {
                                let index = reference.declaration().into_u32() as usize;
                                let Some(target) = module.functions.get(index) else {
                                    return Err(CodegenError(format!(
                                        "function address @{} references invalid local function id {index}",
                                        function.symbol()
                                    )));
                                };
                                let expected = match reference {
                                    scoop_lir::LocalFunctionRef::Managed(_) => GcEffect::Managed,
                                    scoop_lir::LocalFunctionRef::NoGc(_) => GcEffect::NoGc,
                                };
                                if target.gc_effect != expected {
                                    return Err(CodegenError(format!(
                                        "function address @{} has an effect-mismatched local target @{}",
                                        function.symbol(),
                                        target.symbol()
                                    )));
                                }
                            }
                            scoop_lir::FunctionAddressTarget::CallbackTrampoline(bridge)
                                if bridge.into_raw().into_u32() as usize
                                    >= module.callback_bridges.len() =>
                            {
                                return Err(CodegenError(format!(
                                    "function address @{} references invalid callback trampoline {}",
                                    function.symbol(),
                                    bridge.into_raw()
                                )));
                            }
                            scoop_lir::FunctionAddressTarget::CallbackTrampoline(_) => {}
                        }
                    }
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
                                function.symbol(),
                                bridge.into_raw()
                            )));
                        }
                        let bridge = &module.foreign_callback_bridges[*bridge];
                        let family = foreign_callback_family(
                            module,
                            bridge.family,
                            &format!("foreign callback registration @{}", function.symbol()),
                        )?;
                        let closure_ty = checked_value_type(
                            module,
                            function,
                            *closure,
                            "foreign callback registration closure",
                        )?;
                        let result_ty = checked_temp_type(
                            function,
                            *out,
                            "foreign callback registration result",
                        )?;
                        if closure_ty != scoop_lir::MANAGED_PTR
                            || result_ty != &LirType::Struct(family.callback)
                        {
                            return Err(CodegenError(format!(
                                "foreign callback registration @{} for family {} requires a managed closure and exact callback struct {}, got closure {} and result {}",
                                function.symbol(),
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
                            &format!("foreign callback operation @{}", function.symbol()),
                        )?;
                        let callback_ty = checked_value_type(
                            module,
                            function,
                            operation.callback(),
                            "foreign callback operation callback",
                        )?;
                        if callback_ty != LirType::Struct(family.callback) {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} requires exact callback struct {}, got {}",
                                function.symbol(),
                                family_id.into_raw(),
                                family.callback.into_raw(),
                                callback_ty.dump()
                            )));
                        }
                        let result_valid = match *operation {
                            scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                                checked_temp_type(function, out, "foreign callback retain result")?
                                    == &LirType::Struct(family.callback)
                            }
                            scoop_lir::ForeignCallbackOperation::Release { .. } => true,
                            scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                                checked_temp_type(function, out, "foreign callback state result")?
                                    == &LirType::Enum(family.states.definition())
                            }
                            scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                                checked_temp_type(function, out, "foreign callback failure result")?
                                    == &LirType::Enum(family.failure_result.definition())
                            }
                        };
                        if !result_valid {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} has a non-protocol result type",
                                function.symbol(),
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
