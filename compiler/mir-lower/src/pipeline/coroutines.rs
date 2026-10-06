use super::*;

impl Lowerer {
    pub(super) fn materialize_shape_types(
        &mut self,
        module: &hir::Module,
        roots: &[scoop_hir::LocalShapeSupportRoot],
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
                    .expect("a local shape root crosses the HIR to MIR boundary")
                    .identity_record()
                    .id(),
                root.exact(),
                "the local shape plan preserves its exact identity"
            );

            if root.boxed_value() == scoop_hir::LocalBoxedValueRequirement::Required {
                self.boxed
                    .get_or_create(&mut self.classes, &mut self.shell, &value, root.exact());
            }
            let (_, step_ty) = self.coroutines.step_for(
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
            let protocol = self.coroutine_protocol(module, &value);
            self.coroutines.start_helper(
                &self.source_exact_types,
                crate::context::core_provider(module),
                &value,
                self.interfaces.mir_id(protocol.suspend_task),
                self.interfaces.mir_id(protocol.continuation),
                crate::coroutine_registry::protocol_call(
                    module,
                    &self.instances,
                    &self.interfaces,
                    protocol.suspend_task_run,
                ),
                crate::coroutine_registry::protocol_call(
                    module,
                    &self.instances,
                    &self.interfaces,
                    protocol.continuation_resume,
                ),
                crate::coroutine_registry::protocol_call(
                    module,
                    &self.instances,
                    &self.interfaces,
                    protocol.continuation_resume_with_exception,
                ),
                &step_ty,
                crate::coroutine_registry::throwable_type(module, &self.class_map),
                &mut self.functions,
                &mut self.top_level,
                &self.shell,
            );
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
            let lowered_signature =
                crate::coroutine_registry::lowered_signature(module, &source.logical_signature);
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
                lowered_signature,
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
        for protocol in module.coroutine_protocols.iter().copied() {
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
        let protocols = module
            .coroutine_protocols
            .iter()
            .map(|protocol| format!("{:?}", module.types[protocol.result_type].kind))
            .collect::<Vec<_>>();
        panic!(
            "local-concrete HIR provides a protocol for every suspend result type; missing {result:?}, available {protocols:?}"
        )
    }
}
