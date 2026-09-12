use super::*;

pub(super) fn lower_callback_bridges(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> Arena<lir::CallbackBridge> {
    let mut callbacks = Arena::new();
    for (id, callback) in module.callback_bridges.iter() {
        let signature = &module.function_types[callback.signature];
        let lir::LocalFunctionRef::NoGc(bridge) = local_functions[&callback.bridge_function] else {
            unreachable!("validated static callback bridges are NoGC")
        };
        callbacks.alloc(lir::CallbackBridge {
            source_name: module.functions[callback.source].name.clone(),
            bridge,
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
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    callback_signatures: &HashMap<
        mir::FunctionTypeId,
        scoop_identity::CanonicalCAbiSignatureFingerprint,
    >,
) -> Arena<lir::ForeignCallbackBridge> {
    let mut bridges = Arena::new();
    let mut shared_trampolines: HashMap<_, lir::CallbackTrampolineIdentity> = HashMap::new();
    for (_, bridge) in module.foreign_callback_bridges.iter() {
        let signature = &module.function_types[bridge.native_signature];
        let adapter = &module.foreign_callback_adapters[bridge.adapter];
        let signature_fingerprint = callback_signatures[&bridge.native_signature];
        let context_index = scoop_identity::CallbackParameterIndex::new(bridge.context_index);
        let key = (signature_fingerprint, context_index);
        let trampoline = if let Some(identity) = shared_trampolines.get(&key) {
            identity.clone()
        } else {
            let identity = lir::CallbackTrampolineIdentity::new(
                module.cone,
                signature_fingerprint,
                context_index,
            )
            .expect("validated foreign callback bridge identities are encodable");
            shared_trampolines.insert(key, identity.clone());
            identity
        };
        let lir::LocalFunctionRef::Managed(adapter_function) = local_functions[&adapter.function]
        else {
            unreachable!("validated foreign callback adapters are managed")
        };
        bridges.alloc(lir::ForeignCallbackBridge {
            application: bridge.application(),
            family: lir::ForeignCallbackFamilyId::from_raw(bridge.family.into_raw()),
            adapter: adapter_function,
            trampoline,
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
