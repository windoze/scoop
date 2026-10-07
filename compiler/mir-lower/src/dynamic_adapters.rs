//! Fixed dynamic function views and their fully typed managed bodies.

use super::*;

pub(crate) struct DynamicAdapterDefinitions<'a> {
    pub(crate) functions: &'a mut Arena<mir::Function>,
    pub(crate) top_level: &'a mut Vec<mir::FunctionId>,
    pub(crate) classes: &'a mut Arena<mir::ClosureClass>,
    pub(crate) invokes: &'a mut Arena<mir::ClosureInvokeFunction>,
    pub(crate) adapters: &'a mut Arena<mir::DynamicClosureAdapter>,
    pub(crate) by_target: &'a mut HashMap<mir::FunctionTypeId, mir::DynamicClosureAdapterId>,
}

pub(crate) fn request_dynamic_adapter(
    module: &hir::Module,
    shell: &mir::Module,
    target: mir::FunctionTypeId,
    definitions: DynamicAdapterDefinitions<'_>,
) -> mir::DynamicClosureAdapterId {
    if let Some(&adapter) = definitions.by_target.get(&target) {
        return adapter;
    }
    let signature = &shell.function_types[target];
    let name = mir::type_name(shell, &mir::Type::Function(target));
    let function = definitions.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("$dynamic_adapter.{name}"),
        params: Vec::new(),
        return_ty: signature.return_type.clone(),
        body: mir::Body::unreachable(Arena::new()),
    });
    definitions.top_level.push(function);
    let invoke = definitions
        .invokes
        .alloc(mir::ClosureInvokeFunction { function });
    let class = definitions.classes.alloc(mir::ClosureClass {
        name: format!("$Closure$dynamic_adapter${name}"),
        function_type: target,
        invoke,
        captures: vec![mir::Field {
            name: "$source".to_string(),
            ty: mir::Type::Any,
        }],
        bridges: Vec::new(),
    });
    let (signature, exact) = exact_function_identity(module, target);
    let adapter = definitions.adapters.alloc(
        mir::DynamicClosureAdapter::new(class, target, signature, exact)
            .expect("a concrete function view has one complete structural identity"),
    );
    definitions.by_target.insert(target, adapter);
    adapter
}

impl Lowerer {
    pub(crate) fn prepare_function_shapes(
        &mut self,
        module: &hir::Module,
        visited: &mut HashSet<mir::FunctionTypeId>,
    ) {
        let mut pending = self
            .closure_classes
            .iter()
            .flat_map(|(_, class)| {
                [
                    class.function_type,
                    types::dynamic_function_type(module, class.function_type),
                ]
            })
            .chain(self.function_bridge_targets.iter().copied())
            .collect::<Vec<_>>();
        while let Some(function) = pending.pop() {
            if !visited.insert(function) {
                continue;
            }
            let source = &module.function_types[remap_idx(function)];
            self.source_exact_types.record(
                module,
                source.canonical_type,
                mir::Type::Function(function),
            );
            let signature = self.shell.function_types[function].clone();
            let (identity, _) = exact_function_identity(module, function);
            for (ty, exact) in signature
                .parameter_types
                .iter()
                .chain([&signature.return_type])
                .zip(
                    identity
                        .parameters()
                        .iter()
                        .copied()
                        .chain([identity.result()]),
                )
            {
                if let mir::Type::Function(nested) = ty {
                    pending.push(*nested);
                } else if is_boxable(ty) {
                    self.boxed
                        .get_or_create(&mut self.classes, &mut self.shell, ty, exact);
                }
            }
        }
    }

    pub(crate) fn dynamic_view(
        &mut self,
        module: &hir::Module,
        value: smir::Expr,
        target: mir::FunctionTypeId,
    ) -> smir::Expr {
        let adapter = request_dynamic_adapter(
            module,
            &self.shell,
            target,
            DynamicAdapterDefinitions {
                functions: &mut self.functions,
                top_level: &mut self.top_level,
                classes: &mut self.closure_classes,
                invokes: &mut self.closure_invokes,
                adapters: &mut self.dynamic_closure_adapters,
                by_target: &mut self.dynamic_adapter_by_target,
            },
        );
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.dynamic_closure_adapters[adapter].class(),
                captures: vec![smir::ClosureCaptureInit::new(0, value)],
            },
        )
    }

    pub(crate) fn dynamic_box(&mut self, value: smir::Expr) -> smir::Expr {
        if value.ty == mir::Type::Any {
            value
        } else if is_boxable(&value.ty) {
            smir::Expr::new(mir::Type::Any, smir::ExprKind::Box(Box::new(value)))
        } else {
            smir::Expr::new(
                mir::Type::Any,
                smir::ExprKind::Retype {
                    operand: Box::new(value),
                    ty: Box::new(mir::Type::Any),
                },
            )
        }
    }

    pub(crate) fn dynamic_unbox(
        &mut self,
        module: &hir::Module,
        value: smir::Expr,
        target: &mir::Type,
    ) -> smir::Expr {
        if *target == mir::Type::Any {
            value
        } else if let mir::Type::Function(target) = target {
            self.dynamic_view(module, value, *target)
        } else if is_boxable(target) {
            smir::Expr::new(target.clone(), smir::ExprKind::Unbox(Box::new(value)))
        } else {
            smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(value),
                    ty: Box::new(target.clone()),
                },
            )
        }
    }

    pub(crate) fn finalize_dynamic_adapters(&mut self, module: &hir::Module, next: &mut usize) {
        while *next < self.dynamic_closure_adapters.len() {
            let id = mir::DynamicClosureAdapterId::from_raw((*next as u32).into());
            *next += 1;
            let adapter = &self.dynamic_closure_adapters[id];
            let class = adapter.class();
            let target = adapter.target();
            let identity = adapter.identity().clone();
            let function = self.closure_invokes[self.closure_classes[class].invoke].function;
            let signature = self.shell.function_types[target].clone();
            let dynamic = types::dynamic_function_type(module, target);
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
            for (index, ty) in signature.parameter_types.iter().enumerate() {
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
                args.push(self.dynamic_box(smir::Expr::local(local, ty.clone())));
            }
            let call = smir::Expr::new(
                mir::Type::Any,
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::FunctionBridge {
                            function_type: dynamic,
                        },
                        callee: mir::Callee::FunctionBridge(dynamic),
                    },
                    args,
                    return_ty: mir::Type::Any,
                }),
            );
            let span = Span::new(0, 0);
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
                    kind: smir::StatementKind::Return {
                        value: Some(self.dynamic_unbox(module, call, &signature.return_type)),
                    },
                    span,
                }]
            };
            self.functions[function].params = params;
            self.local_values.record_generated_dispatch_parameters(
                function,
                identity.materialization(),
                &self.functions[function].params,
            );
            self.functions[function].body = finish_cfg_body(
                &mut self.local_values,
                &mut self.coroutines,
                function,
                identity.materialization(),
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
            if signature.is_suspend {
                self.suspend_sources.push(SuspendSource {
                    function,
                    materialization: identity.materialization(),
                    odr_group: Some(identity.odr_group_record().id()),
                    logical_signature: identity.callable_signature_record().signature().clone(),
                    source_return: signature.return_type,
                });
            }
        }
    }
}
