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
            let captures: Vec<_> = lambda
                .captures
                .iter()
                .map(|capture| mir::Field {
                    name: capture.name.clone(),
                    ty: types.lower(
                        capture.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                })
                .collect();
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: self.function_map[&lambda.function],
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
                self.closure_capture_indices
                    .insert((class, capture.binding), index as u32);
            }
            self.lambda_closures.insert(id, class);
        }
        for (id, anonymous) in module.anonymous_functions.iter() {
            let function_type = self.lower_function_type_id(module, anonymous.function_type);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let captures: Vec<_> = anonymous
                .captures
                .iter()
                .map(|capture| mir::Field {
                    name: capture.name.clone(),
                    ty: types.lower(
                        capture.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                })
                .collect();
            let invoke = self.closure_invokes.alloc(mir::ClosureInvokeFunction {
                function: self.function_map[&anonymous.function],
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
                self.closure_capture_indices
                    .insert((class, capture.binding), index as u32);
            }
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
            let mut capture_fields =
                Vec::with_capacity(reference.captures.len() + usize::from(receiver.is_some()));
            if let Some(receiver) = receiver {
                capture_fields.push(mir::Field {
                    name: "$receiver".to_string(),
                    ty: types.lower(
                        receiver.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                });
            }
            capture_fields.extend(reference.captures.iter().map(|capture| mir::Field {
                name: capture.name.clone(),
                ty: types.lower(
                    capture.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                ),
            }));
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
                symbol: format!("scoop.$reference.{}", id.into_raw()),
                params: Vec::new(),
                return_ty: signature.return_type.clone(),
                body: mir::Body::unreachable(Arena::new()),
            });
            self.top_level.push(function);
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
            let capture_offset = u32::from(receiver.is_some());
            for (index, capture) in reference.captures.iter().enumerate() {
                self.closure_capture_indices
                    .insert((class, capture.binding), capture_offset + index as u32);
            }
            let mut args = Vec::with_capacity(
                usize::from(receiver.is_some()) + reference.captures.len() + source_args.len(),
            );
            if receiver.is_some() {
                args.push(smir::Expr::new(
                    self.closure_classes[class].captures[0].ty.clone(),
                    smir::ExprKind::ClosureCapture {
                        closure: Box::new(smir::Expr::local(
                            closure,
                            mir::Type::Function(function_type),
                        )),
                        class,
                        index: 0,
                    },
                ));
            } else if matches!(
                &reference.target,
                hir::CallableReferenceTarget::Local { .. }
            ) {
                for index in 0..reference.captures.len() {
                    args.push(smir::Expr::new(
                        self.closure_classes[class].captures[index].ty.clone(),
                        smir::ExprKind::ClosureCapture {
                            closure: Box::new(smir::Expr::local(
                                closure,
                                mir::Type::Function(function_type),
                            )),
                            class,
                            index: index as u32,
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
            let body = cfg::lower(
                smir::Body { locals, statements },
                signature.return_type.clone(),
                &self.enums.defs,
            );
            self.functions[function].params = params;
            self.functions[function].body = body;
            if signature.is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function,
                    source_return: signature.return_type,
                    instance: None,
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
