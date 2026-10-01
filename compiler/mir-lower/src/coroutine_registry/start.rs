use super::*;

impl CoroutineRegistry {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_helper(
        &mut self,
        exact_types: &SourceExactTypeRegistry,
        result: &mir::Type,
        task_interface: mir::InterfaceId,
        continuation_interface: mir::InterfaceId,
        run: mir::CallTarget,
        resume: mir::CallTarget,
        resume_with_exception: mir::CallTarget,
        step_ty: &mir::Type,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        top_level: &mut Vec<mir::FunctionId>,
        shell: &mir::Module,
    ) -> mir::FunctionId {
        let source = Self::source_type(exact_types, result);
        let exact = source.identity_record();
        let nominal_group = source.nominal_specialization();
        let step_id = self
            .steps_by_result
            .iter()
            .find(|(found, _)| *found == exact.id())
            .map(|(_, id)| *id)
            .expect("start helpers are created after their CoroutineStep metadata");
        let step_metadata = &self.steps[step_id];
        assert_eq!(
            step_ty,
            &mir::Type::Enum(step_metadata.enum_id(), Vec::new()),
            "the start helper result and CoroutineStep type must identify the same metadata",
        );
        assert_eq!(
            self.step_metadata_for_type(step_ty).result(),
            result,
            "the start helper CoroutineStep metadata must carry its result type",
        );
        if let Some(found) = self
            .start_helpers
            .iter()
            .find(|found| found.identity().result_record().id() == exact.id())
        {
            return found.function();
        }
        let completed_variant = step_metadata.completed();
        let completed_payload = step_metadata.completed_payload();

        let mut locals = Arena::new();
        let task = locals.alloc(mir::Local {
            name: "task".to_string(),
            ty: mir::Type::Interface(task_interface),
            mutable: false,
        });
        let completion = locals.alloc(mir::Local {
            name: "completion".to_string(),
            ty: mir::Type::Interface(continuation_interface),
            mutable: false,
        });
        let step = locals.alloc(mir::Local {
            name: "$step".to_string(),
            ty: step_ty.clone(),
            mutable: false,
        });
        let exception = locals.alloc(mir::Local {
            name: "$exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let span = mir::SourceSpan::new(0, 0).expect("synthetic span is ordered");
        let statement = |kind| mir::Statement { kind, span };
        let mut blocks = Arena::new();
        let suspended = blocks.alloc(mir::BasicBlock {
            name: "suspended".to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let completed = blocks.alloc(mir::BasicBlock {
            name: "completed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: resume,
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::new(
                            result.clone(),
                            mir::ExprKind::EnumField {
                                operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                                variant: completed_payload.variant().variant_index(),
                                index: completed_payload.field_index(),
                            },
                        ),
                    ],
                    pending: mir::CoroutinePendingContext::Root,
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let failed = blocks.alloc(mir::BasicBlock {
            name: "failed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: resume_with_exception,
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::local(exception, throwable.clone()),
                    ],
                    pending: mir::CoroutinePendingContext::Root,
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let catch_pad = blocks.alloc(mir::BasicBlock {
            name: "body_failure".to_string(),
            statements: vec![
                statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                statement(mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: exception,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                        },
                        args: vec![mir::Expr::caught_exception()],
                        pending: mir::CoroutinePendingContext::Root,
                    },
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            ],
            terminator: mir::Terminator::Goto(failed),
            unwind: None,
        });
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: run,
                        args: vec![
                            mir::Expr::local(task, mir::Type::Interface(task_interface)),
                            mir::Expr::local(
                                completion,
                                mir::Type::Interface(continuation_interface),
                            ),
                        ],
                        pending: mir::CoroutinePendingContext::Root,
                    },
                },
            ))],
            terminator: mir::Terminator::Branch {
                cond: mir::Expr::machine_eq(
                    mir::Expr::enum_tag(mir::Expr::local(step, step_ty.clone())),
                    mir::MachineScalarValue::EnumTag(completed_variant.variant_index()),
                ),
                then_block: completed,
                else_block: suspended,
            },
            unwind: Some(catch_pad),
        });
        let result_name = mir::type_name(shell, result);
        let function = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("startCoroutine<{result_name}>"),
            params: vec![
                mir::Param {
                    name: "task".to_string(),
                    ty: mir::Type::Interface(task_interface),
                    local: task,
                },
                mir::Param {
                    name: "completion".to_string(),
                    ty: mir::Type::Interface(continuation_interface),
                    local: completion,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals,
                blocks,
                entry,
                loop_header_polls: Vec::new(),
            },
        });
        top_level.push(function);
        let signature = hir::ExactCallableSignature::new(
            hir::Effect::Ordinary,
            None,
            [task_interface, continuation_interface]
                .map(|interface| {
                    exact_types
                        .get(&mir::Type::Interface(interface))
                        .expect("a concrete coroutine protocol interface has one exact identity")
                        .identity_record()
                        .id()
                })
                .into(),
            exact_types
                .get(&mir::Type::Unit)
                .expect("the core Unit type has one exact identity")
                .identity_record()
                .id(),
        );
        let identity = mir::CoroutineStartIdentity::new(exact, nominal_group, signature)
            .expect("local-concrete coroutine protocols have one coroutine-start identity");
        let metadata = mir::CoroutineStart::checked(functions, result.clone(), function, identity)
            .expect("generated coroutine start helper has the exact erased signature");
        self.start_helpers.push(metadata);
        function
    }
}
