use super::*;

impl Lowerer {
    pub(crate) fn finalize_function_bridges(&mut self, module: &hir::Module) -> bool {
        let classes: Vec<_> = self.closure_classes.iter().map(|(id, _)| id).collect();
        let targets = self.function_bridge_targets.clone();
        let mut added = false;
        for class in classes {
            let source = self.closure_classes[class].function_type;
            for &target in &targets {
                if self.finalized_function_bridges.contains(&(class, target))
                    || !self.mir_type_is_subtype(
                        module,
                        &mir::Type::Function(source),
                        &mir::Type::Function(target),
                    )
                {
                    continue;
                }
                let (function, identity) = if source == target {
                    let invoke = self.closure_classes[class].invoke;
                    (self.closure_invokes[invoke].function, None)
                } else {
                    let (function, identity) =
                        self.build_function_bridge(module, class, source, target);
                    (function, Some(identity))
                };
                self.closure_classes[class]
                    .bridges
                    .push(mir::FunctionBridge { target, function });
                if let Some(identity) = identity {
                    self.function_bridges.push(
                        mir::FunctionBridgeMaterialization::checked(
                            class,
                            &self.closure_classes[class],
                            target,
                            function,
                            identity,
                        )
                        .expect("a generated function bridge has one physical dispatch entry"),
                    );
                }
                self.finalized_function_bridges.insert((class, target));
                added = true;
            }
        }
        added
    }

    pub(crate) fn build_function_bridge(
        &mut self,
        module: &hir::Module,
        class: mir::ClosureClassId,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> (mir::FunctionId, mir::FunctionBridgeIdentity) {
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let (source_identity, _) = exact_function_identity(module, source);
        let (target_identity, _) = exact_function_identity(module, target);
        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            local: closure,
        }];
        let mut args = vec![smir::Expr::local(closure, mir::Type::Function(source))];
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
                    span: Span { start: 0, end: 0 },
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: Span { start: 0, end: 0 },
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
                span: Span { start: 0, end: 0 },
            }]
        };
        let source_name = &self.closure_classes[class].name;
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target))
            .expect("function bridge targets are source-level MIR types");
        let name = format!("function_bridge.{source_name}.{target_name}");
        let body = cfg::lower(
            smir::Body {
                locals,
                statements,
                coroutine_eh: None,
            },
            target_signature.return_type.clone(),
            &self.enums.defs,
        );
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: target_signature.return_type.clone(),
            body,
        });
        self.top_level.push(function);
        let identity = self.function_bridge_identity(module, class, target_identity);
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                materialization: identity.materialization(),
                odr_group: identity
                    .odr_member_record()
                    .map(|member| member.key().group()),
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        (function, identity)
    }

    fn function_bridge_identity(
        &self,
        module: &hir::Module,
        class: mir::ClosureClassId,
        target: hir::ExactCallableSignature,
    ) -> mir::FunctionBridgeIdentity {
        if let Some(environment) = self
            .closure_environments
            .iter()
            .find(|environment| environment.class() == class)
        {
            let odr_group = match environment.identity().callable().context() {
                hir::CallableMaterializationContext::NoSubstitution => None,
                hir::CallableMaterializationContext::Application(application) => Some(
                    module
                        .callable_applications
                        .odr(application)
                        .expect("a closure materialization references its callable application")
                        .group(),
                ),
                hir::CallableMaterializationContext::InitializationApplication(unit) => {
                    Some(delegated_property_group(module, unit))
                }
            };
            return mir::FunctionBridgeIdentity::new(
                environment.identity().generated_type_record(),
                target,
                odr_group,
            )
            .expect("a concrete source closure environment has one materialization root");
        }
        if let Some((_, adapter)) = self
            .closure_adapters
            .iter()
            .find(|(_, adapter)| adapter.class() == class)
        {
            return mir::FunctionBridgeIdentity::new(
                adapter.identity().environment_record(),
                target,
                Some(adapter.identity().odr_group_record().id()),
            )
            .expect("a static function adapter environment has one structural root");
        }
        if let Some((_, adapter)) = self
            .dynamic_closure_adapters
            .iter()
            .find(|(_, adapter)| adapter.class() == class)
        {
            return mir::FunctionBridgeIdentity::new(
                adapter.identity().environment_record(),
                target,
                Some(adapter.identity().odr_group_record().id()),
            )
            .expect("a dynamic function adapter environment has one structural root");
        }
        unreachable!("every closure class has a persistent environment identity")
    }
}

fn delegated_property_group(
    module: &hir::Module,
    unit: hir::PersistentInitializationUnitId,
) -> hir::OdrGroupId {
    let unit = module
        .initialization_units
        .iter()
        .find_map(|(_, candidate)| (candidate.identity.id() == unit).then_some(candidate))
        .expect("a closure materialization references its initialization unit");
    let hir::InitializationUnitKey::GenericDelegatedExtensionApplication {
        property,
        receiver_arguments,
    } = unit.identity.key()
    else {
        panic!("an initialization closure belongs to a generic delegated extension")
    };
    hir::OdrGroupId::from_key(&hir::SpecializationKey::DelegatedProperty {
        origin: *property,
        receiver_arguments: receiver_arguments.clone(),
    })
    .expect("a delegated-property ODR group identity is hashable")
}
