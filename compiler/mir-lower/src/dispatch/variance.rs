use super::*;

impl Lowerer {
    pub(crate) fn adapt_variance_bridge(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target_type)) = (source, target) {
            let adapter = self.ensure_function_adapter(*source, *target_type);
            return smir::Expr::new(
                mir::Type::Function(*target_type),
                smir::ExprKind::ClosureAlloc {
                    class: self.closure_adapters[adapter].class,
                    captures: vec![value],
                },
            );
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return value;
        }
        if is_boxable(source) && is_reference_mir(target) {
            let boxed = self
                .boxed
                .get_or_create(&mut self.classes, &mut self.shell, source);
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
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let source_name = mir::encode_type(&self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target));
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
            name: format!("$Closure$adapter${source_name}${target_name}"),
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
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
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
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body { locals, statements },
            target_signature.return_type.clone(),
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        adapter
    }

    pub(crate) fn interface_is_subtype(
        &mut self,
        _module: &hir::Module,
        source: mir::InterfaceId,
        target: mir::InterfaceId,
    ) -> bool {
        source == target
    }

    pub(crate) fn mir_type_is_subtype(
        &mut self,
        module: &hir::Module,
        source: &mir::Type,
        target: &mir::Type,
    ) -> bool {
        if source == target || matches!(target, mir::Type::Any) {
            return true;
        }
        match (source, target) {
            (mir::Type::Class(source), mir::Type::Class(target)) => {
                let mut current = Some(*source);
                while let Some(class) = current {
                    if class == *target {
                        return true;
                    }
                    current = self.classes[class].base_class();
                }
                false
            }
            (mir::Type::Interface(source), mir::Type::Interface(target)) => {
                self.interface_is_subtype(module, *source, *target)
            }
            (mir::Type::Class(source), mir::Type::Interface(target)) => {
                let interfaces = self.classes[*source].interfaces.clone();
                interfaces
                    .into_iter()
                    .any(|interface| self.interface_is_subtype(module, interface, *target))
            }
            (mir::Type::Struct(_) | mir::Type::Enum(_, _), mir::Type::Interface(target)) => self
                .value_interfaces(module, source)
                .into_iter()
                .any(|interface| self.interface_is_subtype(module, interface, *target)),
            (mir::Type::Function(source), mir::Type::Function(target)) => {
                let source = self.shell.function_types[*source].clone();
                let target = self.shell.function_types[*target].clone();
                source.is_suspend == target.is_suspend
                    && source.parameter_types.len() == target.parameter_types.len()
                    && target
                        .parameter_types
                        .iter()
                        .zip(&source.parameter_types)
                        .all(|(target, source)| self.mir_type_is_subtype(module, target, source))
                    && self.mir_type_is_subtype(module, &source.return_type, &target.return_type)
            }
            _ => false,
        }
    }
}
