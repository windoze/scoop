use super::*;

mod coroutines;
mod declarations;
mod functions;

impl Lowerer {
    pub(super) fn run(mut self, module: &hir::Module) -> mir::Module {
        // Struct / class / interface ids first (types can reference
        // any of them regardless of declaration order), then the
        // mangling shell (their names for `encode_type`), then the
        // field types themselves — which can instantiate enums.
        self.reserve_nominal_ids(module);
        self.lower_structs(module);
        self.declare_classes(module);
        // Concrete interface type arguments may name classes, so every class
        // id must exist before interface applications are transposed.
        self.lower_interfaces(module);
        self.shell = mangling_shell(
            &self.structs.defs,
            &self.enums.defs,
            &self.classes,
            &self.interfaces.defs,
        );
        self.fill_nominal_type_arguments(module);
        self.lower_function_types(module);
        self.fill_class_hierarchy(module);
        self.fill_struct_fields(module);
        self.lower_globals(module);
        self.lower_singletons(module);
        // Classes are processed base-before-derived: the object layout
        // and the vtable both keep the base's as a prefix.
        let class_order = topo_class_order(module);
        self.fill_class_fields(module, &class_order);
        self.lower_extern_functions(module);

        // Declare every fully concrete user function first, so calls resolve
        // regardless of declaration order. Intrinsics have no body;
        // their callsites map to `Callee::Runtime` shims (see
        // `BodyLowerer::lower_call`). HIR has already closed and instantiated
        // every generic dependency before this stage starts.
        // Member functions are declared too (hir-lower keeps them out of
        // `top_level`). Abstract interface slots become signature-only
        // shells; default bodies and reachable private interface helpers are
        // emitted as ordinary functions with their typed interface owner.
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if !matches!(function.kind, hir::FunctionKind::User(_)) {
                continue;
            }
            let id = self.declare_function(module, hir_id);
            user_functions.push((hir_id, id));
        }
        for (hir_id, function) in module.functions.iter() {
            let Some(method) = function.method else {
                continue;
            };
            if !matches!(module.types[method.owner].kind, hir::TypeKind::Interface(_))
                || !matches!(method.dispatch, hir::MethodDispatch::Direct)
                || !matches!(function.kind, hir::FunctionKind::User(_))
                || self.function_map.contains_key(&hir_id)
            {
                continue;
            }
            let id = self.declare_function(module, hir_id);
            user_functions.push((hir_id, id));
        }
        let mut interface_methods = module
            .interfaces
            .iter()
            .map(|(id, interface)| (id, vec![None; interface.methods.len()]))
            .collect::<HashMap<_, _>>();
        for (hir_id, function) in module.functions.iter() {
            let Some(method) = function.method else {
                continue;
            };
            // The typed dispatch identity is authoritative; MIR does not
            // infer interface ownership from a qualified source name.
            let hir::MethodDispatch::Interface { interface, slot } = method.dispatch else {
                continue;
            };
            let implementation =
                module.interfaces[interface].methods[slot.into_raw() as usize].implementation;
            let mir_id = if implementation == hir::InterfaceMemberImplementation::Body {
                let id = self.declare_function(module, hir_id);
                user_functions.push((hir_id, id));
                id
            } else {
                self.declare_interface_method(module, hir_id)
            };
            let previous = interface_methods
                .get_mut(&interface)
                .expect("the interface method names a local interface")[slot.into_raw() as usize]
                .replace(mir_id);
            assert!(
                previous.is_none(),
                "concrete HIR emits one declaration per interface slot"
            );
        }
        for (hir_id, slots) in interface_methods {
            let mir_id = self.interfaces.mir_id(hir_id);
            let mut methods = Vec::with_capacity(slots.len());
            for (slot, function) in slots.into_iter().enumerate() {
                methods.push(
                    function
                        .unwrap_or_else(|| self.declare_interface_signature(module, hir_id, slot)),
                );
            }
            self.interfaces.defs[mir_id].methods = methods;
        }
        // Bound callable-reference invoke bodies preserve virtual/interface
        // dispatch, so closure materialization needs completed slot tables.
        // Dispatch itself only depends on declared methods, not constructors
        // or lowered source bodies.
        self.compute_dispatch(module, &class_order);
        self.declare_closures(module);
        // Constructor functions are declared from HIR's complete hidden
        // callable arena; MIR does not rediscover instantiability from class
        // modifiers or representation shape.
        let mut ctor_functions = Vec::new();
        for (constructor_id, _) in module.class_constructors.iter() {
            let id = self.declare_ctor(module, constructor_id);
            ctor_functions.push((constructor_id, id));
        }
        let mut struct_ctor_functions = Vec::new();
        for (constructor_id, _) in module.struct_constructors.iter() {
            let id = self.declare_struct_ctor(module, constructor_id);
            struct_ctor_functions.push((constructor_id, id));
        }
        self.lower_initialization_units(module);
        let ensure_units = self
            .initialization_units
            .iter()
            .map(|(unit, declaration)| (declaration.ensure, unit))
            .collect::<HashMap<_, _>>();

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = if let Some(&unit) = ensure_units.get(&mir_id) {
                self.lower_initialization_ensure(module, unit, module.functions[hir_id].span)
            } else {
                self.lower_user_function(module, hir_id)
            };
            let body = cfg::lower(body, return_ty.clone(), &self.enums.defs);
            if module.functions[hir_id].is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function: mir_id,
                    source_return: return_ty.clone(),
                    instance: self.instances.get(hir_id),
                });
            }
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (constructor_id, mir_id) in ctor_functions {
            let (params, return_ty, body) = self.lower_ctor(module, constructor_id);
            let body = cfg::lower(body, return_ty.clone(), &self.enums.defs);
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (constructor_id, mir_id) in struct_ctor_functions {
            let (params, return_ty, body) = self.lower_struct_ctor(module, constructor_id);
            let body = cfg::lower(body, return_ty.clone(), &self.enums.defs);
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (hir_id, function) in module.functions.iter() {
            if function.is_suspend
                && !module.top_level.contains(&hir_id)
                && self.function_map.contains_key(&hir_id)
            {
                let return_ty = self.functions[self.function_map[&hir_id]].return_ty.clone();
                self.suspend_sources.push(SuspendSource {
                    function: self.function_map[&hir_id],
                    source_return: return_ty,
                    instance: self.instances.get(hir_id),
                });
            }
        }

        // Finalize boxed value types to a fixed point. Function-type bridges
        // can discover additional boxed payloads.
        let mut next_boxed = 0;
        loop {
            while next_boxed < self.boxed.order.len() {
                self.finalize_boxed(module, next_boxed);
                next_boxed += 1;
            }
            let added_function_bridges = self.finalize_function_bridges(module);
            if next_boxed == self.boxed.order.len() && !added_function_bridges {
                break;
            }
        }

        self.transform_suspend_abis(module);
        coroutine::transform(&mut self, module);

        // The entry point is a non-generic user function, hence always
        // in the map.
        let entry = self.function_map[&self.entry];
        let boxed_types = self
            .boxed
            .by_type
            .into_iter()
            .map(|(payload, class)| mir::BoxedType { payload, class })
            .collect();
        let option_core = self.enums.all_option_core(module);
        mir::Module {
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            initialization_units: self.initialization_units,
            initialization_failure_roots: self.initialization_failure_roots,
            objects: self.objects,
            object_types: self.object_types,
            singleton_values: self.singleton_values,
            singleton_published_roots: self.singleton_published_roots,
            callback_bridges: self.callback_bridges,
            foreign_callback_adapters: self.foreign_callback_adapters,
            foreign_callback_families: self.foreign_callback_families,
            foreign_callback_bridges: self.foreign_callback_bridges,
            function_types: self.shell.function_types,
            closure_classes: self.closure_classes,
            closure_invoke_functions: self.closure_invokes,
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs.defs,
            enums: self.enums.defs,
            classes: self.classes,
            interfaces: self.interfaces.defs,
            option_core,
            entry,
            meta: mir::MirMeta {
                generic_function_sources: self.instances.generic_function_sources,
                parameterized_method_sources: self.instances.parameterized_method_sources,
                generic_method_sources: self.instances.generic_method_sources,
                instances: self.instances.meta,
                coroutine_functions: self.coroutines.functions,
                coroutine_steps: self.coroutines.steps,
                coroutine_slots: self.coroutines.slots,
                coroutine_saved_values: self.coroutines.saved_values,
                coroutine_failure_values: self.coroutines.failure_values,
                coroutine_frames: self.coroutines.frames,
                coroutine_resume_points: self.coroutines.resume_points,
                closure_adapters: self.closure_adapters,
                dynamic_closure_adapters: self.dynamic_closure_adapters,
                boxed_types,
                ..mir::MirMeta::default()
            },
        }
    }
}
