use super::*;

impl Lowerer {
    /// Generate boxed value types' ordinary interface dispatch. A box has no
    /// universal vtable entries; every interface the value type implements
    /// gets an itable whose slots point at adjust thunks. The thunk's
    /// `this` is the boxed object; it unboxes and tail-calls the real
    /// value method. Concrete HIR supplies the exact implementation function
    /// for each typed interface slot.
    pub(crate) fn finalize_boxed(&mut self, module: &hir::Module, index: usize) {
        let class_id = self.boxed.order[index];
        let payload = self.classes[class_id].declared_fields()[0].ty.clone();
        let encoded = mir::encode_type(&self.shell, &payload)
            .expect("boxed payloads are source-level MIR types");
        debug_assert!(self.classes[class_id].vtable.is_empty());
        let interfaces = self.classes[class_id].interfaces.clone();
        for iface in interfaces {
            let (hir_iface, _) = self.interfaces.source(iface);
            let method_indices: Vec<_> = module.interfaces[hir_iface]
                .methods
                .iter()
                .enumerate()
                .map(|(index, _)| index)
                .collect();
            let mut slots = Vec::new();
            let mut identities = Vec::new();
            for index in method_indices {
                let (thunk, identity) = self.build_thunk(module, &payload, &encoded, iface, index);
                slots.push(mir::TableSlot::Function(thunk));
                identities.push((index, thunk, identity));
            }
            self.classes[class_id].itables.push(mir::ItableRecord {
                interface: iface,
                slots,
            });
            for (slot, function, identity) in identities {
                self.boxing_adjusts.push(
                    mir::BoxingAdjust::checked(
                        &self.functions,
                        &self.classes,
                        &self.interfaces.defs,
                        mir::BoxingAdjustLocation::new(
                            class_id,
                            iface,
                            u32::try_from(slot).expect("interface method indices fit in u32"),
                            function,
                        ),
                        identity,
                    )
                    .expect("a generated boxing adjust occupies its exact itable slot"),
                );
            }
        }
    }

    /// The adjust thunk for one (boxed value type, interface method)
    /// pair (impl spec 2.9): `this` is the boxed object; the thunk
    /// either unboxes it before calling a value method or retypes the box to
    /// the interface receiver expected by a default body. Value-type methods
    /// take `this` by value at MIR; the pointer convention of the receiver is
    /// a codegen ABI matter. The implementation
    /// is selected by its typed concrete-HIR conformance entry, so overloads
    /// never require a name/signature search. The thunk symbol carries the
    /// parameter encoding when the interface overloads the name.
    pub(crate) fn build_thunk(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        encoded: &str,
        iface: mir::InterfaceId,
        method_index: usize,
    ) -> (mir::FunctionId, mir::BoxingAdjustIdentity) {
        let (hir_iface, _) = self.interfaces.source(iface);
        let signature = &module.interfaces[hir_iface].methods[method_index];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: mir::Type::Any,
            local: this,
        }];
        let mut args = Vec::new();
        let mut target_params = Vec::new();
        let mut argument_locals = Vec::new();
        for param in &signature.params {
            let ty = types.lower(
                param.ty,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            target_params.push(ty.clone());
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: param.name.clone(),
                ty,
                local,
            });
            argument_locals.push(local);
        }
        let return_ty = types.lower(
            signature.return_ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let payload_source = self
            .source_exact_types
            .get(payload)
            .expect("boxed payloads retain their local-concrete exact identity");
        let interface_source = self
            .source_exact_types
            .get(&mir::Type::Interface(iface))
            .expect("boxed interfaces retain their local-concrete exact identity");
        let exact_signature = hir::ExactCallableSignature::new(
            if signature.is_suspend {
                hir::Effect::Suspend
            } else {
                hir::Effect::Ordinary
            },
            Some(interface_source.identity_record().id()),
            signature
                .params
                .iter()
                .map(|parameter| module.exact_type_identities[parameter.ty].id())
                .collect(),
            module.exact_type_identities[signature.return_ty].id(),
        );
        let slot = module.dispatch_slot_identities.interface_slot(
            hir_iface,
            hir::InterfaceMethodSlot::from_raw(
                u32::try_from(method_index).expect("interface method indices fit in u32"),
            ),
        );
        let identity = mir::BoxingAdjustIdentity::new(
            payload_source.identity_record(),
            payload_source.nominal_specialization(),
            slot,
            interface_source.identity_record(),
            exact_signature,
        )
        .expect("validated interface slots and exact types form one boxing-adjust identity");
        let implementations = self
            .value_interface_implementations(module, payload)
            .to_vec();
        let source_implementation = implementations
            .into_iter()
            .find(|implementation| {
                let source = self.interfaces.mir_id(implementation.interface);
                self.interface_is_subtype(module, source, iface)
            })
            .expect("concrete HIR supplies the boxed value's target conformance");
        let implementation = source_implementation
            .methods
            .into_iter()
            .find(|implementation| implementation.slot.into_raw() as usize == method_index)
            .expect("concrete HIR supplies every boxed itable slot");
        let implementation = match implementation.target {
            hir::InterfaceImplementationTarget::Method(function) => function,
            hir::InterfaceImplementationTarget::Abstract { .. } => {
                unreachable!("value-type interface implementations are always concrete")
            }
        };
        let implementation_function = &module.functions[implementation];
        let source_types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let receiver_ty = source_types.lower(
            implementation_function.params[0].ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let receiver = if receiver_ty == *payload {
            smir::Expr::new(
                payload.clone(),
                smir::ExprKind::Unbox(Box::new(smir::Expr::local(this, mir::Type::Any))),
            )
        } else {
            assert!(
                matches!(receiver_ty, mir::Type::Interface(_)),
                "a boxed conformance target is a value method or interface default"
            );
            smir::Expr::new(
                receiver_ty.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(smir::Expr::local(this, mir::Type::Any)),
                    ty: Box::new(receiver_ty),
                },
            )
        };
        args.push(receiver);
        let source_params = implementation_function
            .params
            .iter()
            .skip(1)
            .map(|param| {
                source_types.lower(
                    param.ty,
                    &mut self.source_exact_types,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect::<Vec<_>>();
        let implementation_return = source_types.lower(
            implementation_function.return_ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        for (index, ((local, target_ty), source_ty)) in argument_locals
            .into_iter()
            .zip(&target_params)
            .zip(&source_params)
            .enumerate()
        {
            args.push(self.adapt_variance_bridge(
                module,
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                module.exact_type_identities[signature.params[index].ty].id(),
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            implementation_return.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(self.function_map[&implementation]),
                },
                args,
                return_ty: implementation_return.clone(),
            }),
        );
        let kind = if return_ty == mir::Type::Unit {
            smir::StatementKind::Expr(call)
        } else {
            smir::StatementKind::Return {
                value: Some(self.adapt_variance_bridge(
                    module,
                    call,
                    &implementation_return,
                    module.exact_type_identities[implementation_function.return_ty].id(),
                    &return_ty,
                )),
            }
        };
        let encoding = mir::encode_params(&self.shell, &target_params)
            .expect("interface method parameters are source-level MIR types");
        let iface_name = self.interfaces.defs[iface].name.clone();
        let iface_identity = mir::encode_type(&self.shell, &mir::Type::Interface(iface))
            .expect("boxed interface targets have source type encodings");
        // An interface overloading the method name needs the parameter
        // encoding to keep the thunk symbols distinct.
        let overloaded = module.interfaces[hir_iface]
            .methods
            .iter()
            .filter(|sig| sig.name == signature.name)
            .count()
            > 1;
        let name = if overloaded {
            format!("thunk.{encoded}.{iface_name}.{}.{encoding}", signature.name)
        } else {
            format!("thunk.{encoded}.{iface_name}.{}", signature.name)
        };
        let symbol = if overloaded {
            format!(
                "scoop.thunk.{encoded}.{iface_identity}.{}.{encoding}",
                signature.name
            )
        } else {
            format!("scoop.thunk.{encoded}.{iface_identity}.{}", signature.name)
        };
        let body = cfg::lower(
            smir::Body {
                locals,
                statements: vec![smir::Statement {
                    kind,
                    span: signature.span,
                }],
                coroutine_eh: None,
            },
            return_ty.clone(),
            &self.enums.defs,
        );
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol,
            name,
            params,
            return_ty: return_ty.clone(),
            body,
        });
        self.top_level.push(id);
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function: id,
                materialization: identity.materialization(),
                odr_group: identity
                    .root()
                    .member_record()
                    .map(|member| member.key().group()),
                source_return: return_ty,
                instance: None,
            });
        }
        (id, identity)
    }

    pub(crate) fn value_interfaces(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Vec<mir::InterfaceId> {
        let declared = match self.value_struct_source(module, payload) {
            Some(hir_id) => module.structs[hir_id].interfaces.clone(),
            None => match payload {
                mir::Type::Enum(mir_id, _) => {
                    let hir_id = self.enums.hir_ids[mir_id];
                    module.enums[hir_id].interfaces.clone()
                }
                _ => return Vec::new(),
            },
        };
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        declared
            .into_iter()
            .map(|ty| {
                let lowered = types.lower(
                    ty,
                    &mut self.source_exact_types,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let mir::Type::Interface(interface) = lowered else {
                    unreachable!()
                };
                interface
            })
            .collect()
    }

    pub(crate) fn value_interface_implementations<'a>(
        &self,
        module: &'a hir::Module,
        payload: &mir::Type,
    ) -> &'a [hir::InterfaceImplementation] {
        match self.value_struct_source(module, payload) {
            Some(source) => &module.structs[source].interface_implementations,
            None => match payload {
                mir::Type::Enum(id, _) => {
                    &module.enums[self.enums.hir_ids[id]].interface_implementations
                }
                _ => unreachable!("only value types receive boxed interface adapters"),
            },
        }
    }

    /// Exact source declaration for a MIR struct-like payload. Primitive
    /// representations use the typed relation emitted by HIR; ordinary
    /// struct instances use the mandatory MIR→HIR provenance map.
    pub(crate) fn value_struct_source(
        &self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Option<hir::StructId> {
        match payload {
            mir::Type::Integer(kind) => Some(
                module
                    .intrinsic_type_core
                    .integers
                    .owner(raise_integer_kind(*kind)),
            ),
            mir::Type::Boolean => Some(module.intrinsic_type_core.boolean),
            mir::Type::Struct(mir_id) => Some(self.structs.hir_ids[mir_id]),
            _ => None,
        }
    }
}
