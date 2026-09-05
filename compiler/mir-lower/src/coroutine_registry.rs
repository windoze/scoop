use super::*;

#[derive(Clone)]
pub(super) struct SuspendSource {
    pub(super) function: mir::FunctionId,
    pub(super) source_return: mir::Type,
    pub(super) instance: Option<mir::MonomorphizedFunctionId>,
}

#[derive(Default)]
pub(super) struct CoroutineRegistry {
    pub(super) functions: Arena<mir::CoroutineFunction>,
    pub(super) steps: Arena<mir::CoroutineStep>,
    pub(super) steps_by_result: Vec<(mir::Type, mir::CoroutineStepId)>,
    pub(super) slots: Arena<mir::CoroutineSlot>,
    pub(super) slots_by_value: Vec<(mir::Type, mir::CoroutineSlotId)>,
    pub(super) frames: Arena<mir::CoroutineFrame>,
    pub(super) resume_points: Arena<mir::CoroutineResumePoint>,
    pub(super) continuation_shells: Vec<(mir::Type, mir::FunctionId, mir::FunctionId)>,
    pub(super) start_helpers: Vec<(mir::Type, mir::FunctionId)>,
}

impl CoroutineRegistry {
    pub(super) fn step_for(
        &mut self,
        result: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineStepId, mir::Type) {
        if let Some((_, id)) = self
            .steps_by_result
            .iter()
            .find(|(found, _)| found == result)
        {
            let step = &self.steps[*id];
            return (*id, mir::Type::Enum(step.enum_id, Vec::new()));
        }
        let name = format!("CoroutineStep${}", mir::encode_type(shell, result));
        let result_gc_free = mir_type_gc_free(result, structs, enums);
        let variants = vec![
            mir::VariantDef {
                name: "Completed".to_string(),
                gc_free: result_gc_free,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: result.clone(),
                }],
            },
            mir::VariantDef {
                name: "Suspended".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ];
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: result_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            gc_free: result_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the mangling shell mirrors enum ids");
        let id = self.steps.alloc(mir::CoroutineStep {
            enum_id,
            result: result.clone(),
        });
        self.steps_by_result.push((result.clone(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    pub(super) fn step_type_for(&self, result: &mir::Type) -> Option<mir::Type> {
        self.steps_by_result
            .iter()
            .find(|(found, _)| found == result)
            .map(|(_, id)| mir::Type::Enum(self.steps[*id].enum_id, Vec::new()))
    }

    pub(super) fn slot_for(
        &mut self,
        value: &mir::Type,
        structs: &StructRegistry,
        enums: &mut EnumRegistry,
        shell: &mut mir::Module,
    ) -> (mir::CoroutineSlotId, mir::Type) {
        if let Some((_, id)) = self.slots_by_value.iter().find(|(found, _)| found == value) {
            let slot = &self.slots[*id];
            return (*id, mir::Type::Enum(slot.enum_id, Vec::new()));
        }
        let name = format!("CoroutineSlot${}", mir::encode_type(shell, value));
        let value_gc_free = mir_type_gc_free(value, structs, enums);
        let variants = vec![
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: value_gc_free,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: value.clone(),
                }],
            },
        ];
        let enum_id = enums.defs.alloc(mir::EnumDef {
            name: name.clone(),
            gc_free: value_gc_free,
            variants,
        });
        let shell_id = shell.enums.alloc(mir::EnumDef {
            name,
            gc_free: value_gc_free,
            variants: Vec::new(),
        });
        assert_eq!(enum_id, shell_id, "the mangling shell mirrors enum ids");
        let id = self.slots.alloc(mir::CoroutineSlot {
            enum_id,
            value: value.clone(),
        });
        self.slots_by_value.push((value.clone(), id));
        (id, mir::Type::Enum(enum_id, Vec::new()))
    }

    pub(super) fn continuation_shells(
        &mut self,
        result: &mir::Type,
        continuation: mir::InterfaceId,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        shell: &mir::Module,
    ) -> (mir::FunctionId, mir::FunctionId) {
        if let Some((_, resume, failure)) = self
            .continuation_shells
            .iter()
            .find(|(found, _, _)| found == result)
        {
            return (*resume, *failure);
        }
        let encoded = mir::encode_type(shell, result);
        let mut resume_locals = Arena::new();
        let receiver = resume_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let value = resume_locals.alloc(mir::Local {
            name: "value".to_string(),
            ty: result.clone(),
            mutable: false,
        });
        let resume = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resume${encoded}"),
            symbol: format!("scoop.Continuation.resume${encoded}"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "value".to_string(),
                    ty: result.clone(),
                    local: value,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(resume_locals),
        });
        let mut failure_locals = Arena::new();
        let receiver = failure_locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Interface(continuation),
            mutable: false,
        });
        let exception = failure_locals.alloc(mir::Local {
            name: "exception".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let failure = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("Continuation.resumeWithException${encoded}"),
            symbol: format!("scoop.Continuation.resumeWithException${encoded}"),
            params: vec![
                mir::Param {
                    name: "this".to_string(),
                    ty: mir::Type::Interface(continuation),
                    local: receiver,
                },
                mir::Param {
                    name: "exception".to_string(),
                    ty: throwable,
                    local: exception,
                },
            ],
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(failure_locals),
        });
        self.continuation_shells
            .push((result.clone(), resume, failure));
        (resume, failure)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_helper(
        &mut self,
        result: &mir::Type,
        task_interface: mir::InterfaceId,
        continuation_interface: mir::InterfaceId,
        run: mir::MonomorphizedFunctionId,
        resume: mir::MonomorphizedFunctionId,
        resume_with_exception: mir::MonomorphizedFunctionId,
        step_ty: &mir::Type,
        throwable: mir::Type,
        functions: &mut Arena<mir::Function>,
        top_level: &mut Vec<mir::FunctionId>,
        shell: &mir::Module,
    ) -> mir::FunctionId {
        if let Some((_, function)) = self.start_helpers.iter().find(|(found, _)| found == result) {
            return *function;
        }

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
        let span = Span { start: 0, end: 0 };
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
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 0,
                        },
                        callee: mir::Callee::Monomorphized(resume),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::new(
                            result.clone(),
                            mir::ExprKind::EnumField {
                                operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                                variant: 0,
                                index: 0,
                            },
                        ),
                    ],
                },
            )))],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let failed = blocks.alloc(mir::BasicBlock {
            name: "failed".to_string(),
            statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: continuation_interface,
                            slot: 1,
                        },
                        callee: mir::Callee::Monomorphized(resume_with_exception),
                    },
                    args: vec![
                        mir::Expr::local(completion, mir::Type::Interface(continuation_interface)),
                        mir::Expr::local(exception, throwable.clone()),
                    ],
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
                        target: mir::CallTarget {
                            kind: mir::CallKind::Interface {
                                interface: task_interface,
                                slot: 0,
                            },
                            callee: mir::Callee::Monomorphized(run),
                        },
                        args: vec![
                            mir::Expr::local(task, mir::Type::Interface(task_interface)),
                            mir::Expr::local(
                                completion,
                                mir::Type::Interface(continuation_interface),
                            ),
                        ],
                    },
                },
            ))],
            terminator: mir::Terminator::Branch {
                cond: mir::Expr::machine_eq(
                    mir::Expr::enum_tag(mir::Expr::local(step, step_ty.clone())),
                    mir::MachineScalarValue::EnumTag(0),
                ),
                then_block: completed,
                else_block: suspended,
            },
            unwind: Some(catch_pad),
        });
        let encoded = mir::encode_type(shell, result);
        let function = functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("startCoroutine${encoded}"),
            symbol: format!("scoop.coroutine.start${encoded}"),
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
            },
        });
        top_level.push(function);
        self.start_helpers.push((result.clone(), function));
        function
    }
}
