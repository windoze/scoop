use super::*;

impl Lowerer {
    pub(super) fn run(mut self, module: &hir::Module) -> mir::Module {
        // Struct / class / interface ids first (types can reference
        // any of them regardless of declaration order), then the
        // mangling shell (their names for `encode_type`), then the
        // field types themselves — which can instantiate enums.
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
        self.lower_function_types(module);
        self.fill_class_hierarchy(module);
        self.fill_struct_fields(module);
        self.lower_globals(module);
        self.option_variants = option_variants(module);
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
        // Member functions are declared too (hir-lower keeps them out
        // of `top_level`); interface methods become signature-only
        // shells (M6 interfaces have no default implementations).
        let mut user_functions = Vec::new();
        for &hir_id in &module.top_level {
            let function = &module.functions[hir_id];
            if !matches!(function.kind, hir::FunctionKind::User(_)) {
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
            // Interface methods are signature-only shells: dispatch goes
            // through the itable, so their declarations only provide the
            // complete indirect-call signature. The typed dispatch identity
            // is authoritative; MIR does not infer this role from the owner.
            let hir::MethodDispatch::Interface { interface, slot } = method.dispatch else {
                continue;
            };
            let mir_id = self.declare_interface_method(module, hir_id);
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

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = self.lower_user_function(module, hir_id);
            let body = cfg::lower(body, return_ty.clone());
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
            let body = cfg::lower(body, return_ty.clone());
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

        // Finalize boxed value types to a fixed point. Variance and function
        // bridges can discover additional boxed payloads.
        let mut next_boxed = 0;
        loop {
            while next_boxed < self.boxed.order.len() {
                self.finalize_boxed(module, next_boxed);
                next_boxed += 1;
            }
            let added_variance = self.finalize_variance_itables(module);
            let added_function_bridges = self.finalize_function_bridges(module);
            if next_boxed == self.boxed.order.len() && !added_variance && !added_function_bridges {
                break;
            }
        }

        self.transform_suspend_abis(module);
        coroutine::transform(&mut self, module);

        // The entry point is a non-generic user function, hence always
        // in the map.
        let entry = self.function_map[&module.entry];
        let boxed_types = self
            .boxed
            .by_type
            .into_iter()
            .map(|(payload, class)| mir::BoxedType { payload, class })
            .collect();
        mir::Module {
            functions: self.functions,
            extern_functions: self.extern_functions,
            globals: self.globals,
            callback_bridges: self.callback_bridges,
            foreign_callback_adapters: self.foreign_callback_adapters,
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
            entry,
            meta: mir::MirMeta {
                generic_function_sources: self.instances.generic_function_sources,
                parameterized_method_sources: self.instances.parameterized_method_sources,
                generic_method_sources: self.instances.generic_method_sources,
                instances: self.instances.meta,
                coroutine_functions: self.coroutines.functions,
                coroutine_steps: self.coroutines.steps,
                coroutine_slots: self.coroutines.slots,
                coroutine_frames: self.coroutines.frames,
                coroutine_resume_points: self.coroutines.resume_points,
                closure_adapters: self.closure_adapters,
                dynamic_closure_adapters: self.dynamic_closure_adapters,
                boxed_types,
                ..mir::MirMeta::default()
            },
        }
    }

    pub(super) fn lower_extern_functions(&mut self, module: &hir::Module) {
        for (hir_id, extern_) in module.extern_functions.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let params = extern_
                .params
                .iter()
                .map(|&ty| {
                    types.lower(
                        ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                extern_.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let id = self.extern_functions.alloc(mir::ExternFunction {
                source_name: extern_.source_name.clone(),
                native_symbol: extern_.native_symbol.clone(),
                library: extern_.library.clone(),
                abi: match extern_.abi {
                    hir::ExternAbi::C => mir::ExternAbi::C,
                    hir::ExternAbi::Scoop => mir::ExternAbi::Scoop,
                },
                calling_convention: match extern_.calling_convention {
                    hir::CallingConvention::Cdecl => mir::CallingConvention::Cdecl,
                },
                gc_effect: match extern_.gc_effect {
                    hir::GcEffect::Managed => mir::GcEffect::Managed,
                    hir::GcEffect::NoGc => mir::GcEffect::NoGc,
                },
                params,
                return_type,
            });
            self.extern_map.insert(hir_id, id);
        }
    }

    /// Transpose the concrete HIR function-type arena one-to-one. MIR keeps
    /// the same typed identity order, so type lowering never scans signatures
    /// or re-interns an equal shape.
    pub(super) fn lower_function_types(&mut self, module: &hir::Module) {
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        for (source_id, source) in module.function_types.iter() {
            let parameter_types = source
                .parameter_types
                .iter()
                .map(|parameter| {
                    types.lower(
                        *parameter,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            let return_type = types.lower(
                source.return_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let target_id = self.shell.function_types.alloc(mir::FunctionType {
                is_suspend: source.is_suspend,
                parameter_types,
                return_type,
            });
            assert_eq!(source_id.into_raw(), target_id.into_raw());
        }
    }

    pub(super) fn lower_globals(&mut self, module: &hir::Module) {
        for (hir_id, global) in module.globals.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let ty = types.lower(
                global.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let storage = match &global.storage {
                hir::GlobalStorage::Local {
                    thread_local,
                    initializer,
                } => mir::GlobalStorage::Local {
                    thread_local: *thread_local,
                    initializer: lower_global_constant(initializer, &ty, &self.structs.defs),
                },
                hir::GlobalStorage::Extern {
                    library,
                    native_symbol,
                    thread_local,
                } => mir::GlobalStorage::Extern {
                    library: library.clone(),
                    native_symbol: native_symbol.clone(),
                    thread_local: *thread_local,
                },
            };
            let id = self.globals.alloc(mir::Global {
                name: global.name.clone(),
                symbol: mir::mangle_global(&global.name),
                ty,
                mutable: global.mutable,
                storage,
            });
            self.global_map.insert(hir_id, id);
        }
    }

    pub(super) fn transform_suspend_abis(&mut self, module: &hir::Module) {
        for source in std::mem::take(&mut self.suspend_sources) {
            let (step, step_ty) = self.coroutines.step_for(
                &source.source_return,
                &self.structs,
                &mut self.enums,
                &mut self.shell,
            );
            let protocol = self.coroutine_protocol(module, &source.source_return);
            let continuation = self.interfaces.mir_id(protocol.continuation);
            let continuation_ty = mir::Type::Interface(continuation);
            let function = &mut self.functions[source.function];
            let completion = function.body.locals.alloc(mir::Local {
                name: "$completion".to_string(),
                ty: continuation_ty.clone(),
                mutable: false,
            });
            function.params.push(mir::Param {
                name: "$completion".to_string(),
                ty: continuation_ty,
                local: completion,
            });
            for (_, block) in function.body.blocks.iter_mut() {
                if let mir::Terminator::Return { value } = &mut block.terminator {
                    let completed = value.take().unwrap_or_else(mir::Expr::unit);
                    *value = Some(mir::Expr::new(
                        step_ty.clone(),
                        mir::ExprKind::VariantConstruct {
                            variant: 0,
                            fields: vec![completed],
                        },
                    ));
                }
            }
            function.return_ty = step_ty;
            function.symbol = mir::mangle_suspend(&function.symbol);
            if let Some(instance) = source.instance {
                self.instances.meta[instance].symbol = function.symbol.clone();
            }
            self.coroutines.functions.alloc(mir::CoroutineFunction {
                function: source.function,
                source_return: source.source_return,
                step,
                lowering: mir::CoroutineLowering::Immediate,
            });
        }
    }

    pub(super) fn coroutine_protocol(
        &mut self,
        module: &hir::Module,
        result: &mir::Type,
    ) -> hir::CoroutineProtocol {
        for protocol in module.coroutine_protocols.iter().copied() {
            let lowered = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                protocol.result_type,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            if &lowered == result {
                return protocol;
            }
        }
        let protocols = module
            .coroutine_protocols
            .iter()
            .map(|protocol| format!("{:?}", module.types[protocol.result_type].kind))
            .collect::<Vec<_>>();
        panic!(
            "local-concrete HIR provides a protocol for every suspend result type; missing {result:?}, available {protocols:?}"
        )
    }

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
            callback_bridges: &mut self.callback_bridges,
            callback_by_target: &mut self.callback_by_target,
            foreign_callback_adapters: &mut self.foreign_callback_adapters,
            foreign_callback_bridges: &mut self.foreign_callback_bridges,
            foreign_callback_by_registration: &mut self.foreign_callback_by_registration,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            local_map: HashMap::new(),
            constructor_param_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
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
        }
        .lower_function(function, body)
    }
}
