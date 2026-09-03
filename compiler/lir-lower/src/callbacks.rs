use super::*;

pub(super) fn lower_callback_bridges(module: &mir::Module) -> Arena<lir::CallbackBridge> {
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
                .map(|ty| c_ffi_type(module, ty))
                .collect(),
            return_type: c_ffi_type(module, &signature.return_type),
        });
    }
    callbacks
}

pub(super) fn lower_foreign_callback_bridges(
    module: &mir::Module,
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
            adapter_symbol: module.functions[adapter.function].symbol.clone(),
            trampoline_symbol,
            signature_symbol,
            params: signature
                .parameter_types
                .iter()
                .map(|ty| c_ffi_type(module, ty))
                .collect(),
            return_type: c_ffi_type(module, &signature.return_type),
            context_index: bridge.context_index,
            mode: match bridge.mode {
                mir::ForeignCallbackMode::Reusable => lir::ForeignCallbackMode::Reusable,
                mir::ForeignCallbackMode::OneShot => lir::ForeignCallbackMode::OneShot,
            },
        });
    }
    bridges
}
