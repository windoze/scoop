use super::*;

impl Lowerer {
    pub(super) fn declare_references(
        &mut self,
        module: &hir::Module,
        definitions: &mut ClosureDefinitions,
    ) {
        for (id, reference) in module.callable_references.iter() {
            let function_type = self.lower_function_type_id(module, reference.function_type);
            let callable = reference.target.callee();
            let receiver = match &reference.target {
                hir::CallableReferenceTarget::BoundMember { receiver, .. }
                | hir::CallableReferenceTarget::BoundExtension { receiver, .. }
                | hir::CallableReferenceTarget::BoundIntrinsic { receiver, .. } => {
                    Some(receiver.as_ref())
                }
                hir::CallableReferenceTarget::Named(_)
                | hir::CallableReferenceTarget::Local { .. } => None,
            };
            let target = callable.map(|callee| self.lower_reference_callee(module, callee));
            let call_kind = match &reference.target {
                hir::CallableReferenceTarget::BoundMember { receiver, .. } => self
                    .bound_reference_call_kind(
                        module,
                        receiver.ty,
                        callable.expect("a bound member has a callable target"),
                    ),
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
            let definition = match reference.target {
                hir::CallableReferenceTarget::BoundIntrinsic { intrinsic, .. } => {
                    ClosureDefinition::PrimitiveReference(intrinsic)
                }
                _ => ClosureDefinition::Reference {
                    callee: target.expect("an ordinary reference has a callable target"),
                    kind: call_kind.clone(),
                },
            };
            if let Some(class) = definitions.lookup(
                &self.closure_classes,
                &identity,
                function_type,
                &capture_fields,
                &definition,
            ) {
                self.index_closure_captures(class, &identity, &reference.captures);
                self.reference_closures.insert(id, class);
                continue;
            }
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
            self.index_closure_captures(class, &identity, &reference.captures);
            definitions.insert(class, identity.clone(), definition);
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
            let materialization = *reference.identity.materialization();
            let (call, mut statements) =
                if let hir::CallableReferenceTarget::BoundIntrinsic { intrinsic, .. } =
                    reference.target
                {
                    let mut lowerer = self.body_lowerer(
                        module,
                        function,
                        materialization,
                        mir::ImmortalObjectOwner::Callable(materialization),
                    );
                    lowerer.locals = locals;
                    let call =
                        lowerer.lower_primitive_member_values(intrinsic, args, reference.span);
                    locals = lowerer.locals;
                    let statements = lowerer
                        .prelude
                        .into_iter()
                        .map(|kind| smir::Statement {
                            kind,
                            span: reference.span,
                        })
                        .collect::<Vec<_>>();
                    (call, statements)
                } else {
                    (
                        smir::Expr::new(
                            signature.return_type.clone(),
                            smir::ExprKind::Call(smir::Call {
                                target: mir::CallTarget {
                                    kind: call_kind,
                                    callee: target
                                        .expect("an ordinary reference has a callable target"),
                                },
                                args,
                                return_ty: signature.return_type.clone(),
                            }),
                        ),
                        Vec::new(),
                    )
                };
            let statement = if signature.return_type == mir::Type::Unit {
                smir::StatementKind::Expr(call)
            } else {
                smir::StatementKind::Return { value: Some(call) }
            };
            statements.push(smir::Statement {
                kind: statement,
                span: reference.span,
            });
            if signature.return_type == mir::Type::Unit {
                statements.push(smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: reference.span,
                });
            }
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
}
