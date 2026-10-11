use super::primitive::PrimitiveReference;
use super::targets::ReferenceDefinition;
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
            let maybe_intrinsic = maybe_uninit::reference_intrinsic(module, callable);
            let receiver = match &reference.target {
                hir::CallableReferenceTarget::BoundMember { receiver, .. }
                | hir::CallableReferenceTarget::BoundExtension { receiver, .. }
                | hir::CallableReferenceTarget::BoundIntrinsic { receiver, .. } => {
                    Some(receiver.as_ref())
                }
                hir::CallableReferenceTarget::Named(_)
                | hir::CallableReferenceTarget::Local { .. } => None,
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
            let definition = match (maybe_intrinsic, &reference.target) {
                (Some(kind), _) => ReferenceDefinition::MaybeUninit(kind),
                (None, hir::CallableReferenceTarget::BoundIntrinsic { intrinsic, .. }) => {
                    ReferenceDefinition::Primitive(PrimitiveReference::classify(*intrinsic))
                }
                (None, hir::CallableReferenceTarget::Named(callee)) => {
                    ReferenceDefinition::Direct(self.lower_reference_callee(module, *callee).into())
                }
                (None, hir::CallableReferenceTarget::Local { callee, .. }) => {
                    ReferenceDefinition::Direct(
                        self.lower_reference_callee(module, hir::CallableTarget::Local(*callee))
                            .into(),
                    )
                }
                (None, hir::CallableReferenceTarget::BoundMember { callee, .. }) => {
                    ReferenceDefinition::Bound(self.bound_reference_target(module, *callee))
                }
                (None, hir::CallableReferenceTarget::BoundExtension { callee, .. }) => {
                    ReferenceDefinition::Bound(targets::BoundReferenceTarget::Direct(
                        self.lower_reference_callee(module, *callee).into(),
                    ))
                }
            };
            if let Some(class) = definitions.lookup(
                &self.closure_classes,
                &identity,
                function_type,
                &capture_fields,
                &ClosureDefinition::Reference(definition),
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
            definitions.insert(
                class,
                identity.clone(),
                ClosureDefinition::Reference(definition),
            );
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
            let captured = |index: u32| {
                smir::Expr::new(
                    self.closure_classes[class].captures[index as usize]
                        .ty
                        .clone(),
                    smir::ExprKind::ClosureCapture {
                        closure: Box::new(smir::Expr::local(
                            closure,
                            mir::Type::Function(function_type),
                        )),
                        class,
                        index,
                    },
                )
            };
            let load_receiver = || captured(self.closure_receiver_indices[&class]);
            let materialization = *reference.identity.materialization();
            let (call, mut statements) = match definition {
                ReferenceDefinition::Bound(target) => {
                    let source_args =
                        reference_arguments(&signature.parameter_types, &mut locals, &mut params);
                    let call =
                        target.bind(load_receiver(), source_args, signature.return_type.clone());
                    (
                        smir::Expr::new(signature.return_type.clone(), smir::ExprKind::Call(call)),
                        Vec::new(),
                    )
                }
                ReferenceDefinition::Direct(callee) => {
                    let source_args =
                        reference_arguments(&signature.parameter_types, &mut locals, &mut params);
                    let captures = reference.captures.iter().map(|capture| {
                        captured(self.closure_capture_indices[&(class, capture.binding)])
                    });
                    let call = smir::Call {
                        target: mir::CallTarget::Direct(callee),
                        args: captures.chain(source_args).collect(),
                        return_ty: signature.return_type.clone(),
                    };
                    (
                        smir::Expr::new(signature.return_type.clone(), smir::ExprKind::Call(call)),
                        Vec::new(),
                    )
                }
                ReferenceDefinition::Primitive(intrinsic) => {
                    let receiver = load_receiver();
                    let mut lowerer = self.body_lowerer(
                        module,
                        function,
                        materialization,
                        mir::ImmortalObjectOwner::Callable(materialization),
                    );
                    lowerer.locals = locals;
                    let call = match intrinsic {
                        PrimitiveReference::Unary(operation) => lowerer.unary_reference_value(
                            operation,
                            receiver,
                            &signature.return_type,
                        ),
                        PrimitiveReference::Binary(operation) => {
                            let rhs = reference_parameter(
                                0,
                                &signature.parameter_types[0],
                                &mut lowerer.locals,
                                &mut params,
                            );
                            lowerer.binary_reference_value(
                                operation,
                                receiver,
                                rhs,
                                &signature.return_type,
                                reference.span,
                            )
                        }
                    };
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
                }
                ReferenceDefinition::MaybeUninit(kind) => {
                    let operation = match kind {
                        hir::MaybeUninitIntrinsic::Uninit => mir::MaybeUninitOperation::Uninit,
                        hir::MaybeUninitIntrinsic::Initialized => {
                            mir::MaybeUninitOperation::Initialized(Box::new(reference_parameter(
                                0,
                                &signature.parameter_types[0],
                                &mut locals,
                                &mut params,
                            )))
                        }
                        hir::MaybeUninitIntrinsic::AssumeInit => {
                            mir::MaybeUninitOperation::AssumeInit(Box::new(load_receiver()))
                        }
                    };
                    (
                        crate::body::maybe_uninit_value(signature.return_type.clone(), operation),
                        Vec::new(),
                    )
                }
            };
            let statement = if signature.return_type == mir::Type::Unit {
                smir::StatementKind::Expr(call)
            } else {
                smir::StatementKind::Return { value: call }
            };
            statements.push(smir::Statement {
                kind: statement,
                span: reference.span,
            });
            if signature.return_type == mir::Type::Unit {
                statements.push(smir::Statement {
                    kind: smir::StatementKind::Return {
                        value: smir::Expr::unit(),
                    },
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
                    &self.classes,
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

fn reference_parameter(
    index: usize,
    ty: &mir::Type,
    locals: &mut Arena<mir::Local>,
    params: &mut Vec<mir::Param>,
) -> smir::Expr {
    let name = format!("arg{index}");
    let local = locals.alloc(mir::Local {
        name: name.clone(),
        ty: ty.clone(),
        mutable: false,
    });
    params.push(mir::Param {
        name,
        ty: ty.clone(),
        local,
    });
    smir::Expr::local(local, ty.clone())
}

fn reference_arguments(
    types: &[mir::Type],
    locals: &mut Arena<mir::Local>,
    params: &mut Vec<mir::Param>,
) -> Vec<smir::Expr> {
    types
        .iter()
        .enumerate()
        .map(|(index, ty)| reference_parameter(index, ty, locals, params))
        .collect()
}
