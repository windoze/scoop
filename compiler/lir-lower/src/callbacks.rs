use super::*;

pub(super) fn lower_callback_bridges(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Arena<lir::CallbackBridge> {
    let mut callbacks = Arena::new();
    for (id, callback) in module.callback_bridges.iter() {
        let signature = &module.function_types[callback.signature];
        callbacks.alloc(lir::CallbackBridge {
            source_name: module.functions[callback.source].name.clone(),
            bridge_symbol: module.functions[callback.bridge_function].symbol.clone(),
            trampoline_symbol: format!("scoop_c_callback_{}", id.into_raw().into_u32()),
            params: signature
                .parameter_types
                .iter()
                .map(|ty| c_ffi_type(module, structs, enums, ty))
                .collect(),
            return_type: c_return_type(module, structs, enums, &signature.return_type),
        });
    }
    callbacks
}

pub(super) fn lower_foreign_callback_bridges(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Arena<lir::ForeignCallbackBridge> {
    let mut bridges = Arena::new();
    let mut shared_trampolines: HashMap<(mir::FunctionTypeId, u32), (String, String)> =
        HashMap::new();
    for (_, bridge) in module.foreign_callback_bridges.iter() {
        let signature = &module.function_types[bridge.native_signature];
        let adapter = &module.foreign_callback_adapters[bridge.adapter];
        let key = (bridge.native_signature, bridge.context_index);
        let (trampoline_symbol, signature_symbol) =
            if let Some(symbols) = shared_trampolines.get(&key) {
                symbols.clone()
            } else {
                let raw = shared_trampolines.len();
                let symbols = (
                    format!("scoop_foreign_callback_{raw}"),
                    format!("scoop_foreign_callback_signature_{raw}"),
                );
                shared_trampolines.insert(key, symbols.clone());
                symbols
            };
        bridges.alloc(lir::ForeignCallbackBridge {
            application: bridge.application(),
            family: lir::ForeignCallbackFamilyId::from_raw(bridge.family.into_raw()),
            adapter_symbol: module.functions[adapter.function].symbol.clone(),
            trampoline_symbol,
            signature_symbol,
            params: signature
                .parameter_types
                .iter()
                .map(|ty| c_ffi_type(module, structs, enums, ty))
                .collect(),
            return_type: c_return_type(module, structs, enums, &signature.return_type),
            context_index: bridge.context_index,
            mode: variant_ref(enums, bridge.mode),
        });
    }
    bridges
}

pub(super) fn lower_foreign_callback_families(
    module: &mir::Module,
    enums: &lir::EnumDefs,
) -> Arena<lir::ForeignCallbackFamily> {
    let mut families = Arena::new();
    for (id, family) in module.foreign_callback_families.iter() {
        let modes = lir::ForeignCallbackModes::checked(
            enums,
            variant_ref(enums, family.modes.reusable()),
            variant_ref(enums, family.modes.one_shot()),
        )
        .expect("the checked MIR callback modes map to the LIR enum store");
        let states = lir::ForeignCallbackStates::checked(
            enums,
            variant_ref(enums, family.states.registered()),
            variant_ref(enums, family.states.active()),
            variant_ref(enums, family.states.completed()),
            variant_ref(enums, family.states.failed()),
        )
        .expect("the checked MIR callback states map to the LIR enum store");
        let failure_result = lir::ForeignCallbackFailureResult::checked(
            enums,
            variant_field_ref(enums, family.failure_result.some_payload()),
            variant_ref(enums, family.failure_result.none()),
        )
        .expect("the checked MIR callback failure result maps to the LIR enum store");
        let lowered = families.alloc(lir::ForeignCallbackFamily {
            callback: struct_def_id(family.callback),
            modes,
            states,
            failure_result,
        });
        assert_eq!(
            lowered.into_raw(),
            id.into_raw(),
            "foreign callback family ids transpose one-to-one"
        );
    }
    families
}
