use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        for (hir_id, local) in body.locals.iter() {
            let ty = self.lower_type(local.ty);
            let mir_id = self.locals.alloc(mir::Local {
                name: local.name.clone(),
                ty,
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
        }
        let params = function
            .params
            .iter()
            .map(|param| mir::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty),
                local: self.local_map[&param.local],
            })
            .collect();
        if self.current_closure.is_some() {
            self.current_closure_local = function
                .params
                .first()
                .map(|param| self.local_map[&param.local]);
        }
        let return_ty = self.lower_type(function.return_ty);
        let statements = if is_abstract_bodiless(function) {
            // An abstract method (hir-lower materializes it bodiless):
            // every override replaces its vtable slot and the class
            // cannot be instantiated, so the slot is never reached;
            // the emitted function traps like a pure-virtual stub.
            let message =
                self.trap_message(format!("call to abstract method `{}`", fn_name(function)));
            vec![smir::Statement {
                kind: smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                        },
                        args: vec![smir::Expr::new(
                            mir::Type::String,
                            smir::ExprKind::StringConst(message),
                        )],
                        return_ty: mir::Type::Unit,
                    }),
                )),
                span: function.span,
            }]
        } else {
            self.lower_statements(&body.statements)
        };
        (
            params,
            return_ty,
            smir::Body {
                locals: self.locals,
                statements,
            },
        )
    }

    pub(crate) fn lower_type(&mut self, ty: hir::TypeId) -> mir::Type {
        Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        }
        .lower(ty, self.enums, self.structs, self.interfaces, self.shell)
    }

    pub(crate) fn lower_function_type_id(
        &mut self,
        function_type: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = self.module.function_types[function_type].canonical_type;
        let mir::Type::Function(function_type) = self.lower_type(ty) else {
            unreachable!("lowering a function type preserves its category")
        };
        function_type
    }

    pub(super) fn ensure_lambda_closure(&mut self, id: hir::LambdaId) -> mir::ClosureClassId {
        self.lambda_closures[&id]
    }

    pub(super) fn ensure_anonymous_closure(
        &mut self,
        id: hir::AnonymousFunctionId,
    ) -> mir::ClosureClassId {
        self.anonymous_closures[&id]
    }

    pub(super) fn ensure_reference_closure(
        &mut self,
        id: hir::CallableReferenceId,
    ) -> mir::ClosureClassId {
        self.reference_closures[&id]
    }

    pub(super) fn adapt_function_value(
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

    pub(super) fn adapt_mir_subtype(
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

    pub(super) fn ensure_function_adapter(
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
        let source_name = mir::encode_type(self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(self.shell, &mir::Type::Function(target));
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
            },
            target_signature.return_type,
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

    pub(super) fn adapt_checked_function_value(
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

    pub(super) fn ensure_dynamic_function_adapter(
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
        let encoded = mir::encode_type(self.shell, &mir::Type::Function(target));
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
            smir::Body { locals, statements },
            signature.return_type.clone(),
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

    /// A fresh hidden local (`$<prefix>.<n>`), compiler-generated.
    pub(super) fn new_hidden(
        &mut self,
        prefix: &str,
        ty: mir::Type,
        mutable: bool,
    ) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable,
        })
    }

    /// Emit the queued prelude statements (the trap tests of `!!`)
    /// before the statement they belong to.
    pub(super) fn drain_prelude(&mut self, span: Span, out: &mut Vec<smir::Statement>) {
        out.extend(
            self.prelude
                .drain(..)
                .map(|kind| smir::Statement { kind, span }),
        );
    }
}
