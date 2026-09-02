use super::*;

pub(super) struct GeneratedAdapter {
    pub(super) class: mir::ClassId,
    pub(super) point: mir::CoroutineResumePointId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_adapter(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    destination: Option<FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    state: u32,
    failure_state: i64,
    result: &mir::Type,
    safe_latches: Option<(FrameSlot, FrameSlot)>,
) -> GeneratedAdapter {
    let protocol = lowerer.coroutine_protocol(module, result);
    let continuation = lowerer.interfaces.mir_id(protocol.continuation);
    let name = format!("CoroutineAdapter${}${state}", sanitize(source_symbol));
    let mut fields = vec![
        mir::Field {
            name: "frame".to_string(),
            ty: mir::Type::Class(frame_class),
        },
        mir::Field {
            name: "status".to_string(),
            ty: mir::Type::Int,
        },
    ];
    if let Some((success, failure)) = safe_latches.as_ref() {
        fields.push(mir::Field {
            name: "result".to_string(),
            ty: success.slot_ty.clone(),
        });
        fields.push(mir::Field {
            name: "failure".to_string(),
            ty: failure.slot_ty.clone(),
        });
    }
    let class = generated_class(lowerer, name, fields, vec![continuation], Vec::new());
    let resume = generate_resume_method(
        lowerer,
        module,
        class,
        frame_class,
        destination,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        result,
        safe_latches.as_ref().map(|(success, _)| success.clone()),
    );
    let failure = generate_failure_method(
        lowerer,
        module,
        class,
        frame_class,
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        failure_state,
        safe_latches.as_ref().map(|(_, failure)| failure.clone()),
    );
    lowerer.classes[class].itables = vec![mir::ItableRecord {
        interface: continuation,
        slots: vec![
            mir::TableSlot::Function(resume),
            mir::TableSlot::Function(failure),
        ],
    }];
    let point = lowerer
        .coroutines
        .resume_points
        .alloc(mir::CoroutineResumePoint {
            frame,
            state,
            result: result.clone(),
            adapter: class,
            resume,
            resume_with_exception: failure,
        });
    GeneratedAdapter { class, point }
}

#[allow(clippy::too_many_arguments)]
fn generate_resume_method(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    destination: Option<FrameSlot>,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    driver: mir::FunctionId,
    source_symbol: &str,
    state: u32,
    result: &mir::Type,
    latch: Option<FrameSlot>,
) -> mir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let value = locals.alloc(local("value", result.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let adapter_claim = locals.alloc(local("$adapter_claim", mir::Type::Int));
    let frame_claim = locals.alloc(local("$frame_claim", mir::Type::Int));
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        adapter,
        frame_class,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: {
            let mut statements = Vec::new();
            if let Some(destination) = destination.as_ref() {
                statements.push(field_set(
                    adapter_frame(this, adapter, frame_class),
                    destination.field,
                    slot_value(destination, mir::Expr::local(value, result.clone())),
                ));
            }
            statements.push(atomic_field_store(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ));
            statements.push(statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: step,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(driver),
                        },
                        args: vec![
                            adapter_frame(this, adapter, frame_class),
                            mir::Expr::int(i64::from(state)),
                        ],
                    },
                },
            )));
            statements
        },
        terminator: mir::Terminator::Branch {
            cond: is_completed(step, outer_step.clone()),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let invalid_frame = blocks.alloc(mir::BasicBlock {
        name: "invalid_frame".to_string(),
        statements: vec![atomic_field_store(
            mir::Expr::local(this, mir::Type::Class(adapter)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind: None,
    });
    let claim_frame = blocks.alloc(mir::BasicBlock {
        name: "claim_frame".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                adapter_frame(this, adapter, frame_class),
                0,
                i64::from(state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(state),
            ),
            then_block: valid,
            else_block: invalid_frame,
        },
        unwind: None,
    });
    let claim_waiting = blocks.alloc(mir::BasicBlock {
        name: "claim_waiting".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                ADAPTER_WAITING,
                ADAPTER_COMPLETING_SUCCESS,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(adapter_claim, mir::Type::Int),
                ADAPTER_WAITING,
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch.as_ref() {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    latch.field,
                    slot_value(latch, mir::Expr::local(value, result.clone())),
                ),
                atomic_field_store(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    mir::Expr::int(ADAPTER_LATCHED_SUCCESS),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::ValDecl {
                local: adapter_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_COMPLETING_SUCCESS,
                ),
            })],
            terminator: mir::Terminator::Branch {
                cond: int_eq(
                    mir::Expr::local(adapter_claim, mir::Type::Int),
                    ADAPTER_REGISTERING,
                ),
                then_block: latched,
                else_block: claim_waiting,
            },
            unwind: None,
        })
    } else {
        claim_waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("CoroutineAdapter.resume${state}"),
        symbol: format!("{source_symbol}$resume${state}"),
        params: vec![
            mir::Param {
                name: "this".to_string(),
                ty: mir::Type::Class(adapter),
                local: this,
            },
            mir::Param {
                name: "value".to_string(),
                ty: result.clone(),
                local: value,
            },
        ],
        return_ty: mir::Type::Unit,
        body: mir::Body {
            locals,
            blocks,
            entry,
        },
    });
    lowerer.top_level.push(function);
    function
}

#[allow(clippy::too_many_arguments)]
fn generate_failure_method(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    driver: mir::FunctionId,
    source_symbol: &str,
    state: u32,
    failure_state: i64,
    latch: Option<FrameSlot>,
) -> mir::FunctionId {
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let exception = locals.alloc(local("exception", throwable.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let adapter_claim = locals.alloc(local("$adapter_claim", mir::Type::Int));
    let frame_claim = locals.alloc(local("$frame_claim", mir::Type::Int));
    let mut blocks = Arena::new();
    let invalid = protocol_error_block(lowerer, module, &mut locals, &mut blocks, None);
    let exits = drive_exit_blocks(
        lowerer,
        module,
        &mut locals,
        &mut blocks,
        this,
        step,
        adapter,
        frame_class,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
    );
    let valid = blocks.alloc(mir::BasicBlock {
        name: "valid".to_string(),
        statements: vec![
            field_set(
                adapter_frame(this, adapter, frame_class),
                failure_slot.field,
                slot_value(
                    &failure_slot,
                    mir::Expr::local(exception, throwable.clone()),
                ),
            ),
            atomic_field_store(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: step,
                call: mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(driver),
                    },
                    args: vec![
                        adapter_frame(this, adapter, frame_class),
                        mir::Expr::int(failure_state),
                    ],
                },
            })),
        ],
        terminator: mir::Terminator::Branch {
            cond: is_completed(step, outer_step.clone()),
            then_block: exits.completed,
            else_block: exits.suspended,
        },
        unwind: Some(exits.catch_pad),
    });
    let invalid_frame = blocks.alloc(mir::BasicBlock {
        name: "invalid_frame".to_string(),
        statements: vec![atomic_field_store(
            mir::Expr::local(this, mir::Type::Class(adapter)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind: None,
    });
    let claim_frame = blocks.alloc(mir::BasicBlock {
        name: "claim_frame".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                adapter_frame(this, adapter, frame_class),
                0,
                i64::from(state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(state),
            ),
            then_block: valid,
            else_block: invalid_frame,
        },
        unwind: None,
    });
    let claim_waiting = blocks.alloc(mir::BasicBlock {
        name: "claim_waiting".to_string(),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(this, mir::Type::Class(adapter)),
                1,
                ADAPTER_WAITING,
                ADAPTER_COMPLETING_FAILURE,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(adapter_claim, mir::Type::Int),
                ADAPTER_WAITING,
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind: None,
    });
    let entry = if let Some(latch) = latch.as_ref() {
        let latched = blocks.alloc(mir::BasicBlock {
            name: "latched".to_string(),
            statements: vec![
                field_set(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    latch.field,
                    slot_value(latch, mir::Expr::local(exception, throwable.clone())),
                ),
                atomic_field_store(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    mir::Expr::int(ADAPTER_LATCHED_FAILURE),
                ),
            ],
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: vec![statement(mir::StatementKind::ValDecl {
                local: adapter_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(this, mir::Type::Class(adapter)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_COMPLETING_FAILURE,
                ),
            })],
            terminator: mir::Terminator::Branch {
                cond: int_eq(
                    mir::Expr::local(adapter_claim, mir::Type::Int),
                    ADAPTER_REGISTERING,
                ),
                then_block: latched,
                else_block: claim_waiting,
            },
            unwind: None,
        })
    } else {
        claim_waiting
    };
    let function = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("CoroutineAdapter.resumeWithException${state}"),
        symbol: format!("{source_symbol}$resume_exception${state}"),
        params: vec![
            mir::Param {
                name: "this".to_string(),
                ty: mir::Type::Class(adapter),
                local: this,
            },
            mir::Param {
                name: "exception".to_string(),
                ty: throwable,
                local: exception,
            },
        ],
        return_ty: mir::Type::Unit,
        body: mir::Body {
            locals,
            blocks,
            entry,
        },
    });
    lowerer.top_level.push(function);
    function
}

struct DriveExitBlocks {
    completed: mir::BlockId,
    suspended: mir::BlockId,
    catch_pad: mir::BlockId,
}

#[allow(clippy::too_many_arguments)]
fn drive_exit_blocks(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    this: mir::LocalId,
    step: mir::LocalId,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    step_ty: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
) -> DriveExitBlocks {
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let exception = locals.alloc(local("$uncaught", throwable.clone()));
    let completion_ty = mir::Type::Interface(outer_continuation);
    let completed_value_ty = lowerer.functions[outer_resume].params[1].ty.clone();
    let completed = blocks.alloc(mir::BasicBlock {
        name: "completed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                0,
                mir::Expr::int(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: outer_continuation,
                        slot: 0,
                    },
                    callee: mir::Callee::User(outer_resume),
                },
                args: vec![
                    frame_field(
                        adapter_frame(this, adapter, frame_class),
                        1,
                        completion_ty.clone(),
                    ),
                    mir::Expr::new(
                        completed_value_ty,
                        mir::ExprKind::EnumField {
                            operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                            variant: 0,
                            index: 0,
                        },
                    ),
                ],
            }))),
        ],
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let suspended = blocks.alloc(mir::BasicBlock {
        name: "suspended".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let failed = blocks.alloc(mir::BasicBlock {
        name: "failed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                0,
                mir::Expr::int(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: outer_continuation,
                        slot: 1,
                    },
                    callee: mir::Callee::User(outer_failure),
                },
                args: vec![
                    frame_field(adapter_frame(this, adapter, frame_class), 1, completion_ty),
                    mir::Expr::local(exception, throwable.clone()),
                ],
            }))),
        ],
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
    DriveExitBlocks {
        completed,
        suspended,
        catch_pad,
    }
}
