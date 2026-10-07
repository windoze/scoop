use super::*;

impl Lowerer {
    pub(crate) fn adapt_variance_bridge(
        &mut self,
        module: &hir::Module,
        value: smir::Expr,
        source: &mir::Type,
        source_identity: hir::PersistentExactTypeId,
        target: &mir::Type,
    ) -> smir::Expr {
        if source == target
            || matches!(source, mir::Type::Class(id)
                if matches!(self.classes[*id].representation,
                    mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Nothing)))
        {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target_type)) = (source, target) {
            let adapter = self.ensure_function_adapter(module, *source, *target_type);
            return smir::Expr::new(
                mir::Type::Function(*target_type),
                smir::ExprKind::ClosureAlloc {
                    class: self.closure_adapters[adapter].class(),
                    captures: vec![smir::ClosureCaptureInit::new(0, value)],
                },
            );
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return value;
        }
        if is_boxable(source) && is_reference_mir(target) {
            let boxed = self.boxed.get_or_create(
                &mut self.classes,
                &mut self.shell,
                source,
                source_identity,
            );
            if let mir::Type::Interface(interface) = target {
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        unreachable!("variance bridge adaptations always follow a subtype conversion")
    }

    pub(crate) fn ensure_function_adapter(
        &mut self,
        module: &hir::Module,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let source_name = mir::type_name(&self.shell, &mir::Type::Function(source));
        let target_name = mir::type_name(&self.shell, &mir::Type::Function(target));
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$adapter.{source_name}.{target_name}"),
            params: Vec::new(),
            return_ty: target_signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$adapter${source_name}${target_name}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Function(source),
            }],
            bridges: Vec::new(),
        });
        let (source_identity, _) = exact_function_identity(module, source);
        let (target_identity, target_function_type) = exact_function_identity(module, target);
        let adapter = self.closure_adapters.alloc(
            mir::ClosureAdapter::new(
                class,
                source,
                target,
                source_identity.clone(),
                target_identity.clone(),
                target_function_type,
            )
            .expect("concrete HIR function identities match their canonical exact types"),
        );
        self.closure_adapter_by_types
            .insert((source, target), adapter);

        let span = Span::new(0, 0);
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
            args.push(self.adapt_variance_bridge(
                module,
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                target_identity.parameters()[index],
                source_ty,
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
        let statements = if target_signature.return_type == mir::Type::Unit {
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
                    value: Some(self.adapt_variance_bridge(
                        module,
                        call,
                        &source_signature.return_type,
                        source_identity.result(),
                        &target_signature.return_type,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        let owner = self.closure_adapters[adapter].identity().materialization();
        self.local_values.record_generated_dispatch_parameters(
            function,
            owner,
            &self.functions[function].params,
        );
        let body = finish_cfg_body(
            &mut self.local_values,
            &mut self.coroutines,
            function,
            owner,
            cfg::lower(
                smir::Body {
                    locals,
                    statements,
                    coroutine_eh: None,
                },
                target_signature.return_type.clone(),
                &self.enums.defs,
                &self.classes,
            ),
        );
        self.functions[function].body = body;
        if target_signature.is_suspend {
            let identity = self.closure_adapters[adapter].identity();
            self.suspend_sources.push(SuspendSource {
                function,
                materialization: identity.materialization(),
                odr_group: Some(identity.odr_group_record().id()),
                logical_signature: identity.callable_signature_record().signature().clone(),
                source_return: target_signature.return_type,
            });
        }
        adapter
    }
}
