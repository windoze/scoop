use super::*;

impl Lowerer {
    pub(super) fn materialize_core_shape_types(
        &mut self,
        module: &hir::Module,
        roots: &[scoop_hir::LocalCoreShapeSupportRoot],
    ) {
        for root in roots {
            let value = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                root.ty(),
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            assert_eq!(
                self.source_exact_types
                    .get(&value)
                    .expect("a core shape root crosses the HIR to MIR boundary")
                    .identity_record()
                    .id(),
                root.exact(),
                "the local core shape plan preserves its exact identity"
            );

            if root.boxed_value() == scoop_hir::LocalCoreBoxedValueRequirement::Required {
                self.materialize_core_box(module, root.ty(), &value, root.exact());
            }
            self.coroutines.step_for(
                &self.source_exact_types,
                &value,
                &self.structs,
                &mut self.enums,
                &mut self.shell,
            );
            self.coroutines.slot_for(
                &self.source_exact_types,
                &value,
                &self.structs,
                &mut self.enums,
                &mut self.shell,
            );
        }
    }

    fn materialize_core_box(
        &mut self,
        module: &hir::Module,
        source: hir::TypeId,
        payload: &mir::Type,
        exact: hir::PersistentExactTypeId,
    ) {
        let class = self
            .boxed
            .get_or_create(&mut self.classes, &mut self.shell, payload, exact);
        let declared_interfaces: &[hir::TypeId] = match module.types[source].kind {
            hir::TypeKind::Struct(id) => &module.structs[id].interfaces,
            hir::TypeKind::Enum(id) => &module.enums[id].interfaces,
            hir::TypeKind::Unit
            | hir::TypeKind::Integer(_)
            | hir::TypeKind::Boolean
            | hir::TypeKind::Ptr(_)
            | hir::TypeKind::FunPtr(_) => &[],
            _ => unreachable!("only source struct and enum declarations have core boxes"),
        };
        for &interface in declared_interfaces {
            let lowered = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                interface,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let mir::Type::Interface(interface) = lowered else {
                unreachable!("declared nominal interfaces lower to MIR interfaces")
            };
            if !self.classes[class].interfaces.contains(&interface) {
                self.classes[class].interfaces.push(interface);
            }
        }
    }

    pub(super) fn transform_suspend_abis(&mut self, module: &hir::Module) {
        for source in std::mem::take(&mut self.suspend_sources) {
            let (step, step_ty) = self.coroutines.step_for(
                &self.source_exact_types,
                &source.source_return,
                &self.structs,
                &mut self.enums,
                &mut self.shell,
            );
            let completed_variant = self.coroutines.steps[step].completed();
            let protocol = self.coroutine_protocol(module, &source.source_return);
            let continuation = self.interfaces.mir_id(protocol.continuation);
            let continuation_ty = mir::Type::Interface(continuation);
            let function = &mut self.functions[source.function];
            let completion = function.body.locals.alloc(mir::Local {
                name: "$completion".to_string(),
                ty: continuation_ty.clone(),
                mutable: false,
            });
            function.params.push(mir::Param {
                name: "$completion".to_string(),
                ty: continuation_ty,
                local: completion,
            });
            for (_, block) in function.body.blocks.iter_mut() {
                if let mir::Terminator::Return { value } = &mut block.terminator {
                    let completed = value.take().unwrap_or_else(mir::Expr::unit);
                    *value = Some(mir::Expr::new(
                        step_ty.clone(),
                        mir::ExprKind::VariantConstruct {
                            variant: completed_variant,
                            fields: vec![completed],
                        },
                    ));
                }
            }
            function.return_ty = step_ty;
            self.coroutines.functions.alloc(mir::CoroutineFunction {
                function: source.function,
                source: source.materialization,
                source_odr_group: source.odr_group,
                logical_signature: source.logical_signature,
                source_return: source.source_return,
                step,
                lowering: mir::CoroutineLowering::Immediate,
            });
        }
    }

    pub(crate) fn coroutine_protocol(
        &mut self,
        module: &hir::Module,
        result: &mir::Type,
    ) -> hir::CoroutineProtocol {
        for protocol in self.core_protocols.coroutines.iter().copied() {
            let lowered = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                protocol.result_type,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            if &lowered == result {
                return protocol;
            }
        }
        let protocols = self
            .core_protocols
            .coroutines
            .iter()
            .map(|protocol| format!("{:?}", module.types[protocol.result_type].kind))
            .collect::<Vec<_>>();
        panic!(
            "local-concrete HIR provides a protocol for every suspend result type; missing {result:?}, available {protocols:?}"
        )
    }
}
