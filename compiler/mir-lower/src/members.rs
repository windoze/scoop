use super::*;

impl Lowerer {
    /// Declare an interface method as a signature-only shell that is never
    /// emitted. Virtual interface calls name it so LIR receives the complete
    /// indirect-call parameter and return types.
    pub(super) fn declare_interface_method(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut locals = Arena::new();
        let mut source_params = Vec::with_capacity(function.params.len());
        let params = function
            .params
            .iter()
            .map(|param| {
                let ty = types.lower(
                    param.ty,
                    &mut self.source_exact_types,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let local = locals.alloc(mir::Local {
                    name: param.name.clone(),
                    ty: ty.clone(),
                    mutable: false,
                });
                source_params.push((local, param.local));
                mir::Param {
                    name: param.name.clone(),
                    ty,
                    local,
                }
            })
            .collect();
        let return_ty = types.lower(
            function.return_ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(function.attributes.gc_effect),
            symbol,
            name,
            params,
            return_ty: return_ty.clone(),
            body: mir::Body::unreachable(locals),
        });
        self.source_callables.record_function(module, id, hir_id);
        for (local, source) in source_params {
            self.local_values.record(
                id,
                local,
                module.local_value_identities.function_local(hir_id, source),
            );
        }
        self.function_map.insert(hir_id, id);
        self.record_function_instance(module, hir_id, id);
        id
    }

    /// Materialize an interface slot that has no callable use in this cone.
    /// Local-concrete HIR carries its complete signature on the interface
    /// definition, so MIR can still give every slot a typed function entity
    /// without waiting for a call site or reconstructing it from a name.
    pub(super) fn declare_interface_signature(
        &mut self,
        module: &hir::Module,
        interface: hir::InterfaceId,
        slot: usize,
    ) -> mir::FunctionId {
        let declaration = &module.interfaces[interface];
        let method = &declaration.methods[slot];
        let owner = mir::Type::Interface(self.interfaces.mir_id(interface));
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: owner.clone(),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: owner,
            local: this,
        }];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        params.extend(method.params.iter().map(|param| {
            let ty = types.lower(
                param.ty,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            mir::Param {
                name: param.name.clone(),
                ty,
                local,
            }
        }));
        let return_ty = types.lower(
            method.return_ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = format!("{}.{}", declaration.name, method.name);
        let owner = mir::encode_type(
            &self.shell,
            &mir::Type::Interface(self.interfaces.mir_id(interface)),
        )
        .expect("interface signature owners have source type encodings");
        let symbol = format!("scoop.$interface_signature.{owner}.{slot}");
        self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(method.attributes.gc_effect),
            name,
            symbol,
            params,
            return_ty,
            body: mir::Body::unreachable(locals),
        })
    }

    /// Fill the MIR class fields: the base class's (already
    /// flattened) fields come first — the object layout and HIR's
    /// `ClassField` indices follow the same order — then the
    /// constructor properties in declaration order.
    pub(super) fn fill_class_fields(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let decl = &module.classes[hir_id];
            let mir_id = self.class_map[&hir_id];
            if matches!(
                decl.representation,
                hir::ClassRepresentation::Intrinsic { .. }
            ) {
                debug_assert!(decl.base_class().is_none());
                continue;
            }
            let mut fields = match decl.base_class() {
                Some(base) => clone_fields(self.classes[self.class_map[&base]].declared_fields()),
                None => Vec::new(),
            };
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            for field in decl.declared_fields() {
                fields.push(mir::Field {
                    name: field.name.clone(),
                    ty: types.lower(
                        field.ty,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                });
            }
            match &mut self.classes[mir_id].representation {
                mir::ClassRepresentation::Declared {
                    fields: mir_fields, ..
                } => *mir_fields = fields,
                mir::ClassRepresentation::Intrinsic(_) => debug_assert!(fields.is_empty()),
            }
        }
    }

    /// Declare one hidden class initializer. Source constructor identity and
    /// owner specialization are both encoded, so overloads never collide.
    pub(super) fn declare_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::ClassConstructorId,
    ) -> mir::FunctionId {
        let constructor = &module.class_constructors[constructor_id];
        let decl = &module.classes[constructor.class];
        let name = format!("init.{}.$c{}", decl.name, constructor.source_discriminator);
        let owner = mir::encode_type(
            &self.shell,
            &mir::Type::Class(self.class_map[&constructor.class]),
        )
        .expect("constructor owners have source type encodings");
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.init.{owner}.$c{}", constructor.source_discriminator),
            name,
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.ctors.insert(constructor_id, id);
        self.source_callables
            .record_class_constructor(module, id, constructor_id);
        id
    }

    pub(super) fn declare_struct_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::StructConstructorId,
    ) -> mir::FunctionId {
        let constructor = &module.struct_constructors[constructor_id];
        let decl = &module.structs[constructor.structure];
        let name = format!("ctor.{}.$c{}", decl.name, constructor.source_discriminator);
        let owner = mir::encode_type(
            &self.shell,
            &mir::Type::Struct(self.struct_map[&constructor.structure]),
        )
        .expect("struct constructor owners have source type encodings");
        let gc_effect = match &constructor.kind {
            // Primary struct construction only assembles an already-evaluated
            // value. Defaults and source arguments are evaluated by the
            // caller, so the generated constructor has no managed entry.
            hir::StructConstructorKind::Primary => mir::GcEffect::NoGc,
            hir::StructConstructorKind::Secondary { .. } => mir::GcEffect::Managed,
        };
        let id = self.functions.alloc(mir::Function {
            gc_effect,
            symbol: format!("scoop.ctor.{owner}.$c{}", constructor.source_discriminator),
            name,
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.struct_ctors.insert(constructor_id, id);
        self.source_callables
            .record_struct_constructor(module, id, constructor_id);
        id
    }

    /// Lower a fully checked class initializer. The receiver is an explicit
    /// managed parameter and every base/this edge in the concrete body is a
    /// direct call on that same receiver.
    pub(super) fn lower_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::ClassConstructorId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let constructor = module.class_constructors[constructor_id].clone();
        let class = constructor.class;
        let mir_class = self.class_map[&class];
        let mut lowerer = BodyLowerer {
            module,
            source_exact_types: &mut self.source_exact_types,
            local_values: &mut self.local_values,
            current_function: self.ctors[&constructor_id],
            current_materialization: constructor.materialization,
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
            foreign_callback_by_application: &mut self.foreign_callback_by_application,
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
            closure_capture_indices: &self.closure_capture_indices,
            closure_receiver_indices: &self.closure_receiver_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure: None,
            current_closure_local: None,
            current_local_capture_params: HashMap::new(),
            contains_suspend_call: false,
        };
        let receiver_ty = mir::Type::Class(mir_class);
        let receiver = lowerer.locals.alloc(mir::Local {
            name: "this".into(),
            ty: receiver_ty.clone(),
            mutable: false,
        });
        lowerer.local_values.record(
            lowerer.current_function,
            receiver,
            module.local_value_identities.class_receiver(constructor_id),
        );
        lowerer.constructor_receiver = Some(smir::Expr::local(receiver, receiver_ty.clone()));
        let mut params = vec![mir::Param {
            name: "this".into(),
            ty: receiver_ty,
            local: receiver,
        }];
        for (declaration_index, parameter) in constructor.parameters.iter().enumerate() {
            let ty = lowerer.lower_type(parameter.ty);
            let local = lowerer.locals.alloc(mir::Local {
                name: parameter.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            lowerer.local_values.record(
                lowerer.current_function,
                local,
                module
                    .local_value_identities
                    .class_parameter(constructor_id, declaration_index),
            );
            params.push(mir::Param {
                name: parameter.name.clone(),
                ty: ty.clone(),
                local,
            });
            lowerer
                .constructor_param_map
                .insert(parameter.id, smir::Expr::local(local, ty));
        }
        lowerer.allocate_class_locals(constructor_id, constructor.body());
        let statements = lowerer.lower_statements(&constructor.body().statements);
        assert!(
            lowerer.active_loops.is_empty(),
            "class-constructor loop remapping is balanced"
        );
        let coroutine_eh = lowerer.coroutine_eh_mode();
        let body = smir::Body {
            locals: lowerer.locals,
            statements,
            coroutine_eh,
        };
        (params, mir::Type::Unit, body)
    }

    pub(super) fn lower_struct_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::StructConstructorId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let constructor = module.struct_constructors[constructor_id].clone();
        let structure = constructor.structure;
        let mut lowerer = BodyLowerer {
            module,
            source_exact_types: &mut self.source_exact_types,
            local_values: &mut self.local_values,
            current_function: self.struct_ctors[&constructor_id],
            current_materialization: constructor.materialization,
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
            foreign_callback_by_application: &mut self.foreign_callback_by_application,
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
            closure_capture_indices: &self.closure_capture_indices,
            closure_receiver_indices: &self.closure_receiver_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure: None,
            current_closure_local: None,
            current_local_capture_params: HashMap::new(),
            contains_suspend_call: false,
        };
        let return_ty = mir::Type::Struct(lowerer.struct_map[&structure]);
        let mut params = Vec::new();
        let mut arguments = Vec::new();
        for (declaration_index, parameter) in constructor.parameters.iter().enumerate() {
            let ty = lowerer.lower_type(parameter.ty);
            let local = lowerer.locals.alloc(mir::Local {
                name: parameter.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            lowerer.local_values.record(
                lowerer.current_function,
                local,
                module
                    .local_value_identities
                    .struct_parameter(constructor_id, declaration_index),
            );
            params.push(mir::Param {
                name: parameter.name.clone(),
                ty: ty.clone(),
                local,
            });
            let value = smir::Expr::local(local, ty);
            lowerer
                .constructor_param_map
                .insert(parameter.id, value.clone());
            arguments.push(value);
        }
        let statements = match &constructor.kind {
            hir::StructConstructorKind::Primary => vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(smir::Expr::new(
                        return_ty.clone(),
                        smir::ExprKind::StructInit {
                            struct_id: lowerer.struct_map[&structure],
                            args: arguments,
                        },
                    )),
                },
                span: module.structs[structure].span,
            }],
            hir::StructConstructorKind::Secondary {
                target,
                arguments,
                body,
            } => {
                lowerer.allocate_struct_argument_locals(constructor_id, arguments);
                let mut statements = lowerer.lower_statements(&arguments.statements);
                let call_args = arguments
                    .args
                    .iter()
                    .map(|argument| lowerer.lower_expr(argument))
                    .collect();
                let receiver = lowerer.locals.alloc(mir::Local {
                    name: "$this".into(),
                    ty: return_ty.clone(),
                    mutable: false,
                });
                lowerer.local_values.record(
                    lowerer.current_function,
                    receiver,
                    module
                        .local_value_identities
                        .struct_receiver(constructor_id)
                        .expect("secondary constructor receivers have persistent identities"),
                );
                statements.push(smir::Statement {
                    kind: smir::StatementKind::ValDecl {
                        local: receiver,
                        init: smir::Expr::new(
                            return_ty.clone(),
                            smir::ExprKind::Call(smir::Call {
                                target: mir::CallTarget {
                                    kind: mir::CallKind::Direct,
                                    callee: mir::Callee::User(lowerer.struct_ctors[target]),
                                },
                                args: call_args,
                                return_ty: return_ty.clone(),
                            }),
                        ),
                    },
                    span: module.structs[structure].span,
                });
                lowerer.constructor_receiver = Some(smir::Expr::local(receiver, return_ty.clone()));
                lowerer.local_map.clear();
                lowerer.allocate_struct_body_locals(constructor_id, body);
                statements.extend(lowerer.lower_statements(&body.statements));
                statements.push(smir::Statement {
                    kind: smir::StatementKind::Return {
                        value: Some(smir::Expr::local(receiver, return_ty.clone())),
                    },
                    span: module.structs[structure].span,
                });
                statements
            }
        };
        assert!(
            lowerer.active_loops.is_empty(),
            "struct-constructor loop remapping is balanced"
        );
        let coroutine_eh = lowerer.coroutine_eh_mode();
        (
            params,
            return_ty,
            smir::Body {
                locals: lowerer.locals,
                statements,
                coroutine_eh,
            },
        )
    }
}
