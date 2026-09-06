use super::*;

impl BodyLowerer<'_> {
    pub(in crate::body) fn ensure_lambda_closure(
        &mut self,
        id: hir::LambdaId,
    ) -> mir::ClosureClassId {
        self.lambda_closures[&id]
    }

    pub(in crate::body) fn ensure_anonymous_closure(
        &mut self,
        id: hir::AnonymousFunctionId,
    ) -> mir::ClosureClassId {
        self.anonymous_closures[&id]
    }

    pub(in crate::body) fn ensure_reference_closure(
        &mut self,
        id: hir::CallableReferenceId,
    ) -> mir::ClosureClassId {
        self.reference_closures[&id]
    }

    pub(in crate::body) fn adapt_function_value(
        &mut self,
        value: smir::Expr,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        let adapter = self.ensure_function_adapter(source, target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    pub(in crate::body) fn adapt_mir_subtype(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target)) = (source, target) {
            return self.adapt_function_value(value, *source, *target, span);
        }
        if is_boxable(source) && is_reference_mir(target) {
            self.register_boxed(source, None);
            if let mir::Type::Interface(interface) = target {
                let boxed = self.boxed.get_or_create(self.classes, self.shell, source);
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(value),
                    ty: Box::new(target.clone()),
                },
            );
        }
        unreachable!("function adapter conversions follow the HIR subtype relation")
    }

    pub(in crate::body) fn ensure_function_adapter(
        &mut self,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        assert_eq!(
            source_signature.is_suspend, target_signature.is_suspend,
            "ordinary and suspend function types never coerce"
        );
        assert_eq!(
            source_signature.parameter_types.len(),
            target_signature.parameter_types.len(),
            "function variance preserves arity"
        );
        let source_name = mir::encode_type(self.shell, &mir::Type::Function(source))
            .expect("closure adapter sources are source-level MIR types");
        let target_name = mir::encode_type(self.shell, &mir::Type::Function(target))
            .expect("closure adapter targets are source-level MIR types");
        let name = format!("$Closure$adapter${source_name}${target_name}");

        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$adapter.{source_name}.{target_name}"),
            symbol: format!("scoop.$adapter.{source_name}.{target_name}"),
            params: Vec::new(),
            return_ty: target_signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name,
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Function(source),
            }],
            bridges: Vec::new(),
        });
        let adapter = self.closure_adapters.alloc(mir::ClosureAdapter {
            class,
            source,
            target,
        });
        self.closure_adapter_by_types
            .insert((source, target), adapter);

        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Function(source),
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_mir_subtype(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
                span,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let mut statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_mir_subtype(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                        span,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body {
                locals,
                statements: std::mem::take(&mut statements),
                coroutine_eh: None,
            },
            target_signature.return_type,
            &self.enums.defs,
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: self.shell.function_types[target].return_type.clone(),
                instance: None,
            });
        }
        adapter
    }

    pub(in crate::body) fn adapt_checked_function_value(
        &mut self,
        value: smir::Expr,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        let adapter = self.ensure_dynamic_function_adapter(target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.dynamic_closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    pub(in crate::body) fn ensure_dynamic_function_adapter(
        &mut self,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::DynamicClosureAdapterId {
        if let Some(&adapter) = self.dynamic_adapter_by_target.get(&target) {
            return adapter;
        }
        if !self.function_bridge_targets.contains(&target) {
            self.function_bridge_targets.push(target);
        }
        let signature = self.shell.function_types[target].clone();
        let encoded = mir::encode_type(self.shell, &mir::Type::Function(target))
            .expect("dynamic adapter targets are source-level MIR types");
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$dynamic_adapter.{encoded}"),
            symbol: format!("scoop.$dynamic_adapter.{encoded}"),
            params: Vec::new(),
            return_ty: signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$dynamic_adapter${encoded}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Any,
            }],
            bridges: Vec::new(),
        });
        let adapter = self
            .dynamic_closure_adapters
            .alloc(mir::DynamicClosureAdapter { class, target });
        self.dynamic_adapter_by_target.insert(target, adapter);

        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Any,
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
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
            args.push(smir::Expr::local(local, ty));
        }
        let call = smir::Expr::new(
            signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::FunctionBridge {
                        function_type: target,
                    },
                    callee: mir::Callee::FunctionBridge(target),
                },
                args,
                return_ty: signature.return_type.clone(),
            }),
        );
        let statements = if signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return { value: Some(call) },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body {
                locals,
                statements,
                coroutine_eh: None,
            },
            signature.return_type.clone(),
            &self.enums.defs,
        );
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: signature.return_type,
                instance: None,
            });
        }
        adapter
    }
}
