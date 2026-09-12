use super::*;

impl Lowerer {
    /// Materialize the concrete closure classes before lowering any body, so
    /// every creation expression resolves directly to a typed class id.
    pub(super) fn declare_closures(&mut self, module: &hir::Module) {
        for (id, lambda) in module.lambdas.iter() {
            let function_type = self.lower_function_type_id(module, lambda.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let semantic_fields = lambda
                .captures
                .iter()
                .enumerate()
                .map(|(index, capture)| {
                    (
                        capture_source(index),
                        mir::Field {
                            name: capture.name.clone(),
                            ty: types.lower(
                                capture.ty,
                                &mut self.source_exact_types,
                                &mut self.enums,
                                &mut self.structs,
                                &mut self.interfaces,
                                &mut self.shell,
                            ),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let identity = mir::ClosureEnvironmentIdentity::for_lambda(
                module.functions[lambda.function].materialization,
                lambda
                    .captures
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        (
                            capture_source(index),
                            module
                                .local_value_identities
                                .lambda_capture(id, index)
                                .clone(),
                        )
                    })
                    .collect(),
                materialization_odr_group(
                    module,
                    module.functions[lambda.function].materialization,
                ),
            )
            .expect("LocalConcrete lambda captures have complete persistent identities");
            let captures = order_closure_fields(&identity, semantic_fields);
            let invoke_function = self.function_map[&lambda.function];
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: invoke_function,
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$lambda{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(lambda.function, class);
            for (index, capture) in lambda.captures.iter().enumerate() {
                let physical = identity
                    .physical_index(capture_source(index))
                    .expect("every lambda capture has one physical field");
                self.closure_capture_indices
                    .insert((class, capture.binding), physical);
            }
            self.closure_environments.push(
                mir::ClosureEnvironment::checked(class, &self.closure_classes[class], identity)
                    .expect("lambda closure identity covers every physical field"),
            );
            self.lambda_closures.insert(id, class);
        }
        for (id, anonymous) in module.anonymous_functions.iter() {
            let function_type = self.lower_function_type_id(module, anonymous.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let semantic_fields = anonymous
                .captures
                .iter()
                .enumerate()
                .map(|(index, capture)| {
                    (
                        capture_source(index),
                        mir::Field {
                            name: capture.name.clone(),
                            ty: types.lower(
                                capture.ty,
                                &mut self.source_exact_types,
                                &mut self.enums,
                                &mut self.structs,
                                &mut self.interfaces,
                                &mut self.shell,
                            ),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let identity = mir::ClosureEnvironmentIdentity::for_anonymous_function(
                module.functions[anonymous.function].materialization,
                anonymous
                    .captures
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        (
                            capture_source(index),
                            module
                                .local_value_identities
                                .anonymous_function_capture(id, index)
                                .clone(),
                        )
                    })
                    .collect(),
                materialization_odr_group(
                    module,
                    module.functions[anonymous.function].materialization,
                ),
            )
            .expect("LocalConcrete anonymous captures have complete persistent identities");
            let captures = order_closure_fields(&identity, semantic_fields);
            let invoke_function = self.function_map[&anonymous.function];
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: invoke_function,
            });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$anonymous{}", id.into_raw()),
                function_type,
                invoke,
                captures,
                bridges: Vec::new(),
            });
            self.closure_by_function.insert(anonymous.function, class);
            for (index, capture) in anonymous.captures.iter().enumerate() {
                let physical = identity
                    .physical_index(capture_source(index))
                    .expect("every anonymous-function capture has one physical field");
                self.closure_capture_indices
                    .insert((class, capture.binding), physical);
            }
            self.closure_environments.push(
                mir::ClosureEnvironment::checked(class, &self.closure_classes[class], identity)
                    .expect("anonymous closure identity covers every physical field"),
            );
            self.anonymous_closures.insert(id, class);
        }
        for (id, reference) in module.callable_references.iter() {
            let function_type = self.lower_function_type_id(module, reference.function_type);
            let (callable, receiver) = match &reference.target {
                hir::CallableReferenceTarget::Named(callable) => (*callable, None),
                hir::CallableReferenceTarget::Local { callee, .. } => (*callee, None),
                hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
                hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                    (*callee, Some(receiver.as_ref()))
                }
            };
            let target = self.lower_reference_callee(module, callable);
            let call_kind = match &reference.target {
                hir::CallableReferenceTarget::BoundMember { receiver, .. } => {
                    self.bound_reference_call_kind(module, receiver.ty, callable)
                }
                _ => mir::CallKind::Direct,
            };
            let signature = self.shell.function_types[function_type].clone();
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let mut semantic_fields =
                Vec::with_capacity(reference.captures.len() + usize::from(receiver.is_some()));
            let mut identity_inputs =
                Vec::with_capacity(reference.captures.len() + usize::from(receiver.is_some()));
            if let Some(receiver) = receiver {
                semantic_fields.push((
                    mir::ClosureFieldSource::CallableReferenceReceiver,
                    mir::Field {
                        name: "$receiver".to_string(),
                        ty: types.lower(
                            receiver.ty,
                            &mut self.source_exact_types,
                            &mut self.enums,
                            &mut self.structs,
                            &mut self.interfaces,
                            &mut self.shell,
                        ),
                    },
                ));
                identity_inputs.push((
                    mir::ClosureFieldSource::CallableReferenceReceiver,
                    module
                        .local_value_identities
                        .callable_reference_receiver(id)
                        .expect("a bound callable reference has a persistent receiver identity")
                        .clone(),
                ));
            }
            semantic_fields.extend(reference.captures.iter().enumerate().map(
                |(index, capture)| {
                    (
                        capture_source(index),
                        mir::Field {
                            name: capture.name.clone(),
                            ty: types.lower(
                                capture.ty,
                                &mut self.source_exact_types,
                                &mut self.enums,
                                &mut self.structs,
                                &mut self.interfaces,
                                &mut self.shell,
                            ),
                        },
                    )
                },
            ));
            identity_inputs.extend(reference.captures.iter().enumerate().map(|(index, _)| {
                (
                    capture_source(index),
                    module
                        .local_value_identities
                        .callable_reference_capture(id, index)
                        .clone(),
                )
            }));
            let identity = mir::ClosureEnvironmentIdentity::for_callable_reference(
                *reference.identity.materialization(),
                identity_inputs,
                materialization_odr_group(module, *reference.identity.materialization()),
            )
            .expect("LocalConcrete callable-reference fields have persistent identities");
            let capture_fields = order_closure_fields(&identity, semantic_fields);
            let closure_ty = mir::Type::Function(function_type);
            let mut locals = Arena::new();
            let closure = locals.alloc(mir::Local {
                name: "$closure".to_string(),
                ty: closure_ty.clone(),
                mutable: false,
            });
            let mut params = vec![mir::Param {
                name: "$closure".to_string(),
                ty: closure_ty,
                local: closure,
            }];
            let mut source_args = Vec::with_capacity(signature.parameter_types.len());
            for (index, ty) in signature.parameter_types.iter().cloned().enumerate() {
                let local = locals.alloc(mir::Local {
                    name: format!("arg{index}"),
                    ty: ty.clone(),
                    mutable: false,
                });
                params.push(mir::Param {
                    name: format!("arg{index}"),
                    ty: ty.clone(),
                    local,
                });
                source_args.push(smir::Expr::local(local, ty));
            }
            let function = self.functions.alloc(mir::Function {
                gc_effect: mir::GcEffect::Managed,
                name: format!("$reference.{}", id.into_raw()),
                params: Vec::new(),
                return_ty: signature.return_type.clone(),
                body: mir::Body::unreachable(Arena::new()),
            });
            self.top_level.push(function);
            self.source_callables
                .record_callable_reference(module, function, id);
            let invoke = self
                .closure_invokes
                .alloc(mir::ClosureInvokeFunction { function });
            let class = self.closure_classes.alloc(mir::ClosureClass {
                name: format!("$Closure$reference{}", id.into_raw()),
                function_type,
                invoke,
                captures: capture_fields,
                bridges: Vec::new(),
            });
            for (index, capture) in reference.captures.iter().enumerate() {
                let physical = identity
                    .physical_index(capture_source(index))
                    .expect("every callable-reference capture has one physical field");
                self.closure_capture_indices
                    .insert((class, capture.binding), physical);
            }
            if receiver.is_some() {
                let physical = identity
                    .physical_index(mir::ClosureFieldSource::CallableReferenceReceiver)
                    .expect("every bound callable reference has one receiver field");
                self.closure_receiver_indices.insert(class, physical);
            }
            self.closure_environments.push(
                mir::ClosureEnvironment::checked(class, &self.closure_classes[class], identity)
                    .expect("callable-reference identity covers every physical field"),
            );
            let mut args = Vec::with_capacity(
                usize::from(receiver.is_some()) + reference.captures.len() + source_args.len(),
            );
            if receiver.is_some() {
                let receiver_index = self.closure_receiver_indices[&class];
                args.push(smir::Expr::new(
                    self.closure_classes[class].captures[receiver_index as usize]
                        .ty
                        .clone(),
                    smir::ExprKind::ClosureCapture {
                        closure: Box::new(smir::Expr::local(
                            closure,
                            mir::Type::Function(function_type),
                        )),
                        class,
                        index: receiver_index,
                    },
                ));
            } else if matches!(
                &reference.target,
                hir::CallableReferenceTarget::Local { .. }
            ) {
                for capture in &reference.captures {
                    let physical = self.closure_capture_indices[&(class, capture.binding)];
                    args.push(smir::Expr::new(
                        self.closure_classes[class].captures[physical as usize]
                            .ty
                            .clone(),
                        smir::ExprKind::ClosureCapture {
                            closure: Box::new(smir::Expr::local(
                                closure,
                                mir::Type::Function(function_type),
                            )),
                            class,
                            index: physical,
                        },
                    ));
                }
            } else {
                debug_assert!(reference.captures.is_empty());
            }
            args.extend(source_args);
            let call = smir::Expr::new(
                signature.return_type.clone(),
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: call_kind,
                        callee: target,
                    },
                    args,
                    return_ty: signature.return_type.clone(),
                }),
            );
            let statement = if signature.return_type == mir::Type::Unit {
                smir::StatementKind::Expr(call)
            } else {
                smir::StatementKind::Return { value: Some(call) }
            };
            let mut statements = vec![smir::Statement {
                kind: statement,
                span: reference.span,
            }];
            if signature.return_type == mir::Type::Unit {
                statements.push(smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: reference.span,
                });
            }
            let materialization = *reference.identity.materialization();
            self.local_values.record_generated_dispatch_parameters(
                function,
                materialization,
                &params,
            );
            let body = finish_cfg_body(
                &mut self.local_values,
                &mut self.coroutines,
                function,
                materialization,
                cfg::lower(
                    smir::Body {
                        locals,
                        statements,
                        coroutine_eh: None,
                    },
                    signature.return_type.clone(),
                    &self.enums.defs,
                ),
            );
            self.functions[function].params = params;
            self.functions[function].body = body;
            if signature.is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function,
                    materialization,
                    odr_group: materialization_odr_group(module, materialization),
                    logical_signature: exact_function_type_signature(
                        module,
                        reference.function_type,
                    ),
                    source_return: signature.return_type,
                });
            }
            self.reference_closures.insert(id, class);
        }
    }

    pub(super) fn lower_reference_callee(
        &mut self,
        _module: &hir::Module,
        callable: hir::Callable,
    ) -> mir::Callee {
        let hir::Callable::Function(function) = callable;
        self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        )
    }

    pub(super) fn bound_reference_call_kind(
        &mut self,
        module: &hir::Module,
        _receiver_ty: hir::TypeId,
        callable: hir::Callable,
    ) -> mir::CallKind {
        let function = module.callable_function(callable);
        let declaration = &module.functions[function];
        let method = declaration
            .method
            .expect("a bound member reference names method metadata");
        match method.dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                mir::CallKind::Direct
            }
            hir::MethodDispatch::Virtual(family) => {
                let hir::TypeKind::Class(class) = module.types[method.owner].kind else {
                    unreachable!("a virtual family belongs to a class method")
                };
                let slot = self.method_slots[&self.class_map[&class]][&family];
                mir::CallKind::Virtual { slot }
            }
            hir::MethodDispatch::Interface { interface, slot } => mir::CallKind::Interface {
                interface: self.interfaces.mir_id(interface),
                slot: slot.into_raw(),
            },
        }
    }
}

fn capture_source(index: usize) -> mir::ClosureFieldSource {
    mir::ClosureFieldSource::Capture {
        declaration_index: u32::try_from(index)
            .expect("a closure capture declaration index fits the identity schema"),
    }
}

fn order_closure_fields(
    identity: &mir::ClosureEnvironmentIdentity,
    fields: Vec<(mir::ClosureFieldSource, mir::Field)>,
) -> Vec<mir::Field> {
    let mut by_source = fields.into_iter().collect::<HashMap<_, _>>();
    let physical = identity
        .fields()
        .iter()
        .map(|field| {
            by_source
                .remove(&field.source())
                .expect("every persistent closure field has one physical definition")
        })
        .collect();
    assert!(
        by_source.is_empty(),
        "every physical closure field has a persistent identity"
    );
    physical
}
