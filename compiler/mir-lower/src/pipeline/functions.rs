use super::*;

impl Lowerer {
    /// Lower one fully concrete user function.
    pub(super) fn lower_user_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let function = &module.functions[hir_id];
        let hir::FunctionKind::User(body) = &function.kind else {
            unreachable!("only user functions have MIR bodies")
        };
        let current_local_capture_params = module
            .local_functions
            .iter()
            .find(|(_, local)| local.function == hir_id)
            .map(|(_, local)| {
                local
                    .captures
                    .iter()
                    .zip(function.params.iter())
                    .map(|(capture, param)| (capture.binding, param.local))
                    .collect()
            })
            .unwrap_or_default();
        let current_closure = self.closure_by_function.get(&hir_id).copied();
        BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interfaces: &mut self.interfaces,
            structs: &mut self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            extern_map: &self.extern_map,
            global_map: &self.global_map,
            singleton_root_map: &self.singleton_root_map,
            singleton_published_roots: &self.singleton_published_roots,
            callback_bridges: &mut self.callback_bridges,
            callback_by_target: &mut self.callback_by_target,
            foreign_callback_adapters: &mut self.foreign_callback_adapters,
            foreign_callback_families: &mut self.foreign_callback_families,
            foreign_callback_family_by_callback: &mut self.foreign_callback_family_by_callback,
            foreign_callback_bridges: &mut self.foreign_callback_bridges,
            foreign_callback_by_registration: &mut self.foreign_callback_by_registration,
            ctors: &self.ctors,
            struct_ctors: &self.struct_ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            local_map: HashMap::new(),
            active_loops: Vec::new(),
            next_loop_id: 0,
            constructor_param_map: HashMap::new(),
            constructor_receiver: None,
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            coroutines: &mut self.coroutines,
            lambda_closures: &self.lambda_closures,
            anonymous_closures: &self.anonymous_closures,
            reference_closures: &self.reference_closures,
            closure_classes: &mut self.closure_classes,
            closure_invokes: &mut self.closure_invokes,
            closure_capture_indices: &mut self.closure_capture_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure,
            current_closure_local: None,
            current_local_capture_params,
            contains_suspend_call: false,
        }
        .lower_function(function, body)
    }
}
