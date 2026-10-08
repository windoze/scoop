use super::*;

struct Target {
    callee: mir::Callee,
    materialization: hir::CallableMaterialization,
    parameters: Vec<mir::Type>,
    result: mir::Type,
    gc_effect: mir::GcEffect,
}

impl Lowerer {
    pub(super) fn interface_reference_entry(
        &mut self,
        module: &hir::Module,
        owner: hir::ClassId,
        interface: hir::InterfaceId,
        position: hir::InterfaceMethodSlot,
        source: hir::InterfaceImplementationTarget,
    ) -> mir::TableSlot {
        let target = self.interface_reference_target(module, source);
        if !matches!(target.parameters.first(), Some(mir::Type::Interface(_))) {
            return match target.callee {
                mir::Callee::User(id) => mir::TableSlot::Function(id),
                mir::Callee::External(id) => mir::TableSlot::External(id),
                _ => unreachable!("interface implementations are ordinary Scoop callables"),
            };
        }
        let class = self.class_map[&owner];
        let iface = self.interfaces.mir_id(interface);
        let method = &module.interfaces[interface].methods[position.into_raw() as usize];
        let lowered = self.interfaces.defs[iface].methods[position.into_raw() as usize].clone();
        let exact = &module.exact_type_identities[module.classes[owner].canonical_type];
        let signature = hir::ExactCallableSignature::new(
            if method.is_suspend {
                hir::Effect::Suspend
            } else {
                hir::Effect::Ordinary
            },
            Some(exact.id()),
            method
                .params
                .iter()
                .map(|param| module.exact_type_identities[param.ty].id())
                .collect(),
            module.exact_type_identities[method.return_ty].id(),
        );
        let identity = mir::InterfaceAdjustIdentity::reference(
            exact,
            module
                .exact_type_identities
                .nominal_specialization(module.classes[owner].canonical_type),
            module
                .dispatch_slot_identities
                .interface_slot(interface, position),
            target.materialization,
            signature,
        )
        .expect("a reference adapter has a concrete owner and target");
        if let Some(existing) = self.interface_adjusts.iter().find(|adjust| {
            adjust.identity().callable_record().id() == identity.callable_record().id()
        }) {
            return mir::TableSlot::Function(existing.function());
        }
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".into(),
            ty: mir::Type::Class(class),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".into(),
            ty: mir::Type::Class(class),
            local: this,
        }];
        let mut args = vec![smir::Expr::new(
            target.parameters[0].clone(),
            smir::ExprKind::Retype {
                operand: Box::new(smir::Expr::local(this, mir::Type::Class(class))),
                ty: Box::new(target.parameters[0].clone()),
            },
        )];
        for (index, ty) in lowered.parameters.iter().skip(1).enumerate() {
            let local = locals.alloc(mir::Local {
                name: method.params[index].name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: method.params[index].name.clone(),
                ty: ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                module,
                smir::Expr::local(local, ty.clone()),
                ty,
                module.exact_type_identities[method.params[index].ty].id(),
                &target.parameters[index + 1],
            ));
        }
        let call = smir::Expr::new(
            target.result.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: target.callee,
                },
                args,
                return_ty: target.result.clone(),
            }),
        );
        let statements = if lowered.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span: method.span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: method.span,
                },
            ]
        } else {
            let target_exact = self
                .source_exact_types
                .get(&target.result)
                .expect("a reference target has a complete result identity")
                .identity_record()
                .id();
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_variance_bridge(
                        module,
                        call,
                        &target.result,
                        target_exact,
                        &lowered.return_type,
                    )),
                },
                span: method.span,
            }]
        };
        let body = cfg::lower(
            smir::Body {
                locals,
                statements,
                coroutine_eh: None,
            },
            lowered.return_type.clone(),
            &self.enums.defs,
            &self.classes,
        );
        let function = self.functions.alloc(mir::Function {
            name: format!(
                "receiver<{}> {}.{}",
                self.classes[class].name, self.interfaces.defs[iface].name, method.name
            ),
            gc_effect: target.gc_effect,
            params,
            return_ty: lowered.return_type.clone(),
            body: body.body,
        });
        self.local_values.record_generated_dispatch_parameters(
            function,
            identity.materialization(),
            &self.functions[function].params,
        );
        self.local_values.record_generated(
            function,
            identity.materialization(),
            &body.generated_values,
        );
        self.coroutines.record_call_sites(function, body.call_sites);
        self.top_level.push(function);
        if method.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                materialization: identity.materialization(),
                odr_group: identity
                    .root()
                    .member_record()
                    .map(|member| member.key().group()),
                logical_signature: identity.signature_record().signature().clone(),
                source_return: lowered.return_type,
            });
        }
        let target = match target.callee {
            mir::Callee::User(id) => mir::InterfaceAdjustTarget::Local(id),
            mir::Callee::External(id) => mir::InterfaceAdjustTarget::External(id),
            _ => unreachable!("an interface adapter forwards to an ordinary callable"),
        };
        self.interface_adjusts.push(mir::InterfaceAdjust::new(
            mir::InterfaceAdjustLocation::new(class, iface, position.into_raw(), function),
            target,
            identity,
        ));
        mir::TableSlot::Function(function)
    }

    fn interface_reference_target(
        &mut self,
        module: &hir::Module,
        source: hir::InterfaceImplementationTarget,
    ) -> Target {
        match source {
            hir::InterfaceImplementationTarget::Method(id)
            | hir::InterfaceImplementationTarget::Abstract { declaration: id } => {
                let function = self.function_map[&id];
                let source = &module.functions[id];
                let types = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                };
                let mut lower = |ty| {
                    types.lower(
                        ty,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                };
                // Dispatch precedes body lowering; the concrete HIR signature
                // is already complete, while the MIR declaration has no body.
                let parameters = source.params.iter().map(|param| lower(param.ty)).collect();
                let result = lower(source.return_ty);
                Target {
                    callee: mir::Callee::User(function),
                    materialization: source.materialization,
                    parameters,
                    result,
                    gc_effect: self.functions[function].gc_effect,
                }
            }
            hir::InterfaceImplementationTarget::Imported(id)
            | hir::InterfaceImplementationTarget::ImportedAbstract { declaration: id } => {
                let imported = self.imported_dependency_callable_map[&id].clone();
                let callable = imported.scoop_entry();
                let source = self.external_callables[callable];
                let template = match source.reference().implementation() {
                    scoop_identity::StrongCallableDefinitionOwner::Function(id) => {
                        hir::CallableTemplateOwner::Function(id)
                    }
                    scoop_identity::StrongCallableDefinitionOwner::PropertyAccessor(id) => {
                        hir::CallableTemplateOwner::Accessor(id)
                    }
                    scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(id) => {
                        hir::CallableTemplateOwner::Generated(id)
                    }
                    scoop_identity::StrongCallableDefinitionOwner::Constructor(_) => {
                        unreachable!("a constructor is not an interface method")
                    }
                };
                let types = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                };
                let mut lower = |exact| {
                    types.lower(
                        module
                            .exact_type_identities
                            .type_for_identity(exact)
                            .expect("an imported method retains its signature types"),
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                };
                let parameters = imported
                    .signature
                    .receiver()
                    .into_option()
                    .into_iter()
                    .chain(imported.signature.parameters().iter().copied())
                    .map(&mut lower)
                    .collect();
                let result = lower(imported.signature.result());
                Target {
                    callee: mir::Callee::External(callable),
                    materialization: hir::CallableMaterialization::new(
                        template,
                        hir::CallableMaterializationContext::NoSubstitution,
                    ),
                    parameters,
                    result,
                    gc_effect: source.gc_effect(),
                }
            }
        }
    }
}
