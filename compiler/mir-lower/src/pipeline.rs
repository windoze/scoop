use super::*;

use std::collections::BTreeMap;

mod coroutines;
mod declarations;
mod functions;

impl Lowerer {
    pub(super) fn run(
        mut self,
        module: &hir::Module,
        shape_support: &[scoop_hir::LocalShapeSupportRoot],
    ) -> mir::Module {
        // Struct / class / interface ids first (types can reference
        // any of them regardless of declaration order), then the temporary
        // type context, then the field types themselves — which can
        // instantiate enums.
        self.reserve_nominal_ids(module);
        self.lower_structs(module);
        self.declare_classes(module);
        // Concrete interface type arguments may name classes, so every class
        // id must exist before interface applications are transposed.
        self.lower_interfaces(module);
        self.shell = type_context(
            module.cone,
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
        // `top_level`). Closed abstract interface declarations emit the same
        // fatal bodies as abstract class methods. Generic interface shells
        // retain their separate application materialization path.
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
            let Some(method) = function.receiver.method() else {
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
            .collect::<BTreeMap<_, _>>();
        for (hir_id, function) in module.functions.iter() {
            let Some(method) = function.receiver.method() else {
                continue;
            };
            // The typed dispatch identity is authoritative; MIR does not
            // infer interface ownership from a qualified source name.
            let hir::MethodDispatch::Interface { interface, slot } = method.dispatch else {
                continue;
            };
            let implementation =
                module.interfaces[interface].methods[slot.into_raw() as usize].implementation;
            let mir_id = if implementation == hir::InterfaceMemberImplementation::Body
                || module.interfaces[interface].type_arguments.is_empty()
            {
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
        let mut initialization_string_owners = HashMap::new();
        for (_, unit) in module.initialization_units.iter() {
            let owner = mir::ImmortalObjectOwner::InitializationUnit(unit.identity.id());
            for function in [unit.initializer, unit.ensure] {
                assert!(
                    initialization_string_owners
                        .insert(function, owner)
                        .is_none(),
                    "one concrete function cannot materialize multiple initialization units"
                );
            }
        }
        let ensure_units = self
            .initialization_units
            .iter()
            .map(|(unit, declaration)| (declaration.ensure, unit))
            .collect::<HashMap<_, _>>();

        for (hir_id, mir_id) in user_functions {
            let (params, return_ty, body) = if let Some(&unit) = ensure_units.get(&mir_id) {
                self.lower_initialization_ensure(module, unit, module.functions[hir_id].span)
            } else {
                let string_owner = initialization_string_owners
                    .get(&hir_id)
                    .copied()
                    .unwrap_or(mir::ImmortalObjectOwner::Callable(
                        module.functions[hir_id].materialization,
                    ));
                self.lower_user_function(module, hir_id, mir_id, string_owner)
            };
            let body = finish_cfg_body(
                &mut self.local_values,
                &mut self.coroutines,
                mir_id,
                module.functions[hir_id].materialization,
                cfg::lower(body, return_ty.clone(), &self.enums.defs),
            );
            if module.functions[hir_id].is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function: mir_id,
                    materialization: module.functions[hir_id].materialization,
                    odr_group: materialization_odr_group(
                        module,
                        module.functions[hir_id].materialization,
                    ),
                    logical_signature: exact_function_signature(module, hir_id),
                    source_return: return_ty.clone(),
                });
            }
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (constructor_id, mir_id) in ctor_functions {
            let (params, return_ty, body) = self.lower_ctor(module, constructor_id);
            let body = finish_cfg_body(
                &mut self.local_values,
                &mut self.coroutines,
                mir_id,
                module.class_constructors[constructor_id].materialization,
                cfg::lower(body, return_ty.clone(), &self.enums.defs),
            );
            let function = &mut self.functions[mir_id];
            function.params = params;
            function.return_ty = return_ty;
            function.body = body;
        }
        for (constructor_id, mir_id) in struct_ctor_functions {
            let (params, return_ty, body) = self.lower_struct_ctor(module, constructor_id);
            let body = finish_cfg_body(
                &mut self.local_values,
                &mut self.coroutines,
                mir_id,
                module.struct_constructors[constructor_id].materialization,
                cfg::lower(body, return_ty.clone(), &self.enums.defs),
            );
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
                    materialization: module.functions[hir_id].materialization,
                    odr_group: materialization_odr_group(
                        module,
                        module.functions[hir_id].materialization,
                    ),
                    logical_signature: exact_function_signature(module, hir_id),
                    source_return: return_ty,
                });
            }
        }

        self.materialize_shape_types(module, shape_support);

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

        // A logical callable signature is itself a HIR -> MIR type use. This
        // includes hidden class-initializer receivers and source result types
        // that a later physical ABI transform may erase or replace.
        // Owned language builtins are required exports even without source uses.
        for source in self
            .source_callables
            .required_types(module)
            .into_iter()
            .chain(crate::types::owned_builtin_types(module))
            .chain(std::iter::once(module.string))
        {
            Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                source,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
        }

        self.transform_suspend_abis(module);
        coroutine::transform(&mut self, module);

        let output = match self.output {
            LoweringOutput::Library => mir::MirOutput::Library,
            // The executable entry is a non-generic user function, hence
            // always in the map.
            LoweringOutput::Executable(entry) => mir::MirOutput::Executable {
                entry: self.function_map[&entry],
            },
        };
        let boxed_entries = std::mem::take(&mut self.boxed.entries);
        let boxed_types: Vec<mir::BoxedType> = boxed_entries
            .into_iter()
            .map(|entry| {
                let ty = module
                    .exact_type_identities
                    .type_for_identity(entry.payload_identity)
                    .expect("boxed exact identities come from local-concrete HIR");
                let exact = &module.exact_type_identities[ty];
                if let Some(group) = module.exact_type_identities.nominal_specialization(ty) {
                    mir::BoxedType::for_nominal_application(
                        entry.payload,
                        entry.class,
                        exact,
                        group,
                    )
                } else if matches!(&entry.payload, mir::Type::Tuple(_)) {
                    mir::BoxedType::for_tuple(entry.payload, entry.class, exact)
                } else {
                    mir::BoxedType::for_source_nominal(entry.payload, entry.class, exact)
                }
                .expect("boxable HIR exact types have a boxed-value materialization root")
            })
            .collect();
        let generated_exact_types = self.generated_exact_types(&boxed_types);
        let option_core = self.enums.all_option_core(module);
        let mut output = mir::Module {
            cone: module.cone,
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
            strings: self.strings.finish(),
            structs: self.structs.defs,
            enums: self.enums.defs,
            classes: self.classes,
            interfaces: self.interfaces.defs,
            option_core,
            output,
            meta: mir::MirMeta {
                external_callables: self.external_callables,
                source_exact_types: self.source_exact_types.finish(),
                generated_exact_types,
                source_callable_materializations: self.source_callables.finish(),
                local_values: self.local_values.finish(),
                closure_environments: self.closure_environments,
                function_bridges: self.function_bridges,
                instances: self.instances.meta,
                coroutine_functions: self.coroutines.functions,
                coroutine_steps: self.coroutines.steps,
                coroutine_slots: self.coroutines.slots,
                continuation_shells: self.coroutines.continuation_shells,
                coroutine_starts: self.coroutines.start_helpers,
                coroutine_saved_values: self.coroutines.saved_values,
                coroutine_failure_values: self.coroutines.failure_values,
                coroutine_frames: self.coroutines.frames,
                coroutine_resume_points: self.coroutines.resume_points,
                closure_adapters: self.closure_adapters,
                dynamic_closure_adapters: self.dynamic_closure_adapters,
                boxed_types,
                boxing_adjusts: self.boxing_adjusts,
                ..mir::MirMeta::default()
            },
        };
        output.meta.generated_callables = generated_callables(&output);
        output.meta.callable_signatures = callable_signatures(&output);
        output
    }

    fn generated_exact_types(
        &self,
        boxed_types: &[mir::BoxedType],
    ) -> mir::GeneratedExactTypeIdentities {
        let mut entries = Vec::new();
        let mut register = |location, nominal, odr_member| {
            entries.push(
                mir::GeneratedExactTypeIdentity::new(location, nominal, odr_member)
                    .expect("MIR-generated nominal identity matches its physical arena"),
            );
        };

        for environment in &self.closure_environments {
            register(
                mir::GeneratedExactTypeLocation::Closure(environment.class()),
                environment.identity().generated_type_record(),
                environment.identity().odr_member_record(),
            );
        }
        for (_, adapter) in self.closure_adapters.iter() {
            register(
                mir::GeneratedExactTypeLocation::Closure(adapter.class()),
                adapter.identity().environment_record(),
                Some(adapter.identity().environment_member_record()),
            );
        }
        for (_, adapter) in self.dynamic_closure_adapters.iter() {
            register(
                mir::GeneratedExactTypeLocation::Closure(adapter.class()),
                adapter.identity().environment_record(),
                Some(adapter.identity().environment_member_record()),
            );
        }
        for (_, step) in self.coroutines.steps.iter() {
            register(
                mir::GeneratedExactTypeLocation::Enum(step.enum_id()),
                step.identity().generated_type_record(),
                step.identity().root().member_record(),
            );
        }
        for (_, slot) in self.coroutines.slots.iter() {
            register(
                mir::GeneratedExactTypeLocation::Enum(slot.enum_id()),
                slot.identity().generated_type_record(),
                slot.identity().root().member_record(),
            );
        }
        for (_, frame) in self.coroutines.frames.iter() {
            register(
                mir::GeneratedExactTypeLocation::Class(frame.class()),
                frame.identity().generated_type_record(),
                frame.identity().odr_member_record(),
            );
        }
        for (_, point) in self.coroutines.resume_points.iter() {
            register(
                mir::GeneratedExactTypeLocation::Class(point.adapter()),
                point.identity().generated_type_record(),
                point.identity().odr_member_record(),
            );
        }
        for boxed in boxed_types {
            register(
                mir::GeneratedExactTypeLocation::Class(boxed.class()),
                boxed.identity().generated_type_record(),
                boxed.identity().root().member_record(),
            );
        }

        mir::GeneratedExactTypeIdentities::checked(entries)
            .expect("MIR-generated nominal locations and identities are globally unique")
    }
}

fn generated_callables(module: &mir::Module) -> mir::MirGeneratedCallableIdentities {
    let mut entries = Vec::new();
    let mut register = |function, identity, signature_subject| {
        entries.push(mir::MirGeneratedCallableIdentity::new(
            function,
            identity,
            signature_subject,
        ));
    };

    for (_, bridge) in module.callback_bridges.iter() {
        register(
            bridge.bridge_function,
            bridge.identity().callable_record(),
            bridge.identity().signature_record().subject(),
        );
    }
    for (_, adapter) in module.foreign_callback_adapters.iter() {
        register(
            adapter.function,
            adapter.identity_record(),
            adapter.signature_subject(),
        );
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        register(
            closure_invoke_function(module, adapter.class()),
            adapter.identity().callable_record(),
            adapter.identity().callable_signature_record().subject(),
        );
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        register(
            closure_invoke_function(module, adapter.class()),
            adapter.identity().callable_record(),
            adapter.identity().callable_signature_record().subject(),
        );
    }
    for bridge in &module.meta.function_bridges {
        register(
            bridge.function(),
            bridge.identity().callable_record(),
            bridge.identity().signature_record().subject(),
        );
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        if let mir::CoroutineLowering::StateMachine {
            driver,
            driver_identity,
            ..
        } = &coroutine.lowering
        {
            register(
                *driver,
                driver_identity.callable_record(),
                driver_identity.signature_record().subject(),
            );
        }
    }
    for shell in &module.meta.continuation_shells {
        register(
            shell.success(),
            shell.identity().success_callable_record(),
            shell.identity().success_signature_record().subject(),
        );
        register(
            shell.failure(),
            shell.identity().failure_callable_record(),
            shell.identity().failure_signature_record().subject(),
        );
    }
    for start in &module.meta.coroutine_starts {
        register(
            start.function(),
            start.identity().callable_record(),
            start.identity().signature_record().subject(),
        );
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        register(
            point.resume(),
            point.identity().success().callable_record(),
            point.identity().success().signature_record().subject(),
        );
        register(
            point.resume_with_exception(),
            point.identity().failure().callable_record(),
            point.identity().failure().signature_record().subject(),
        );
    }
    for adjust in &module.meta.boxing_adjusts {
        register(
            adjust.function(),
            adjust.identity().callable_record(),
            adjust.identity().signature_record().subject(),
        );
    }

    mir::MirGeneratedCallableIdentities::checked(entries)
        .expect("MIR-generated callable functions and identities are globally unique")
}

fn callable_signatures(module: &mir::Module) -> mir::MirCallableSignatures {
    let mut entries = Vec::new();
    let mut register = |record: &mir::CallableSignatureRecord| entries.push(record.clone());

    for source in module.meta.source_callable_materializations.iter() {
        register(source.signature_record());
    }
    for (_, bridge) in module.callback_bridges.iter() {
        register(bridge.identity().signature_record());
    }
    for (_, bridge) in module.foreign_callback_bridges.iter() {
        let record = mir::CallableSignatureRecord::new(
            bridge.application_record.managed_adapter(),
            bridge.application_record.managed_signature().clone(),
        );
        register(&record);
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        register(adapter.identity().callable_signature_record());
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        register(adapter.identity().callable_signature_record());
    }
    for bridge in &module.meta.function_bridges {
        register(bridge.identity().signature_record());
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        if let mir::CoroutineLowering::StateMachine {
            driver_identity, ..
        } = &coroutine.lowering
        {
            register(driver_identity.signature_record());
        }
    }
    for shell in &module.meta.continuation_shells {
        register(shell.identity().success_signature_record());
        register(shell.identity().failure_signature_record());
    }
    for start in &module.meta.coroutine_starts {
        register(start.identity().signature_record());
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        register(point.identity().success().signature_record());
        register(point.identity().failure().signature_record());
    }
    for adjust in &module.meta.boxing_adjusts {
        register(adjust.identity().signature_record());
    }

    mir::MirCallableSignatures::checked(entries)
        .expect("MIR callable signature subjects are globally unique")
}

fn closure_invoke_function(module: &mir::Module, class: mir::ClosureClassId) -> mir::FunctionId {
    module.closure_invoke_functions[module.closure_classes[class].invoke].function
}
