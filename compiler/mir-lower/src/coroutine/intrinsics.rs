use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn rewrite_intrinsic_site(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    body: &mut mir::Body,
    frame_local: mir::LocalId,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    site: SuspendSite,
    mut call: mir::Call,
    post: mir::BlockId,
    unwind: Option<mir::BlockId>,
) -> GeneratedSite {
    let SuspendKind::Intrinsic { register } = site.kind else {
        unreachable!("intrinsic site carries its concrete register method")
    };
    let throwable = mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    let (_, result_latch_ty) = lowerer.coroutines.slot_for(
        &site.result,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let (_, failure_latch_ty) = lowerer.coroutines.slot_for(
        &throwable,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let result_latch = FrameSlot {
        field: 2,
        slot_ty: result_latch_ty,
        value_ty: site.result.clone(),
    };
    let failure_latch = FrameSlot {
        field: 3,
        slot_ty: failure_latch_ty,
        value_ty: throwable.clone(),
    };
    let adapter = generate_adapter(
        lowerer,
        module,
        frame_class,
        frame,
        site.destination.map(|local| frame_slots[&local].clone()),
        failure_slot.clone(),
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        source_symbol,
        driver,
        site.state,
        failure_state(site.state),
        &site.result,
        Some((result_latch.clone(), failure_latch.clone())),
    );
    let adapter_local = body.locals.alloc(local(
        &format!("$safe_adapter.{}", site.state),
        mir::Type::Class(adapter.class),
    ));
    let exception = body.locals.alloc(local(
        &format!("$resume_exception.{}", site.state),
        throwable.clone(),
    ));
    let registration_claim = body.locals.alloc(local(
        &format!("$registration_claim.{}", site.state),
        mir::Type::Int,
    ));
    let frame_claim = body.locals.alloc(local(
        &format!("$registration_frame_claim.{}", site.state),
        mir::Type::Int,
    ));

    let current = &mut body.blocks[site.block];
    for local in site
        .live_after
        .iter()
        .copied()
        .filter(|local| Some(*local) != site.destination)
    {
        if let Some(slot) = frame_slots.get(&local) {
            current.statements.push(save_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                mir::Expr::local(local, body.locals[local].ty.clone()),
                slot,
            ));
        }
    }
    current.statements.push(atomic_field_store(
        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
        0,
        mir::Expr::int(i64::from(site.state)),
    ));
    current.statements.extend(initialized_generated_class(
        adapter_local,
        adapter.class,
        vec![
            mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
            mir::Expr::int(ADAPTER_REGISTERING),
            slot_empty(&result_latch),
            slot_empty(&failure_latch),
        ],
    ));
    call.target.callee = mir::Callee::Monomorphized(register);
    call.args.push(mir::Expr::local(
        adapter_local,
        mir::Type::Class(adapter.class),
    ));
    let register_call = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.register.{}", site.state),
        statements: vec![statement(mir::StatementKind::Call(mir::CallEffect::Unit(
            call,
        )))],
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    body.blocks[site.block].terminator = mir::Terminator::Goto(register_call);

    let success = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_success.{}", site.state),
        statements: {
            let mut statements = vec![atomic_field_store(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            )];
            if let Some(destination) = site.destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::new(
                        site.result.clone(),
                        mir::ExprKind::EnumField {
                            operand: Box::new(frame_field(
                                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                                result_latch.field,
                                result_latch.slot_ty.clone(),
                            )),
                            variant: 1,
                            index: 0,
                        },
                    ),
                }));
            }
            statements
        },
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    let failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_failure.{}", site.state),
        statements: vec![
            atomic_field_store(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                mir::Expr::int(ADAPTER_CONSUMED),
            ),
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::new(
                    throwable.clone(),
                    mir::ExprKind::EnumField {
                        operand: Box::new(frame_field(
                            mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                            failure_latch.field,
                            failure_latch.slot_ty.clone(),
                        )),
                        variant: 1,
                        index: 0,
                    },
                ),
            }),
        ],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, throwable.clone()),
            unwind,
        },
        unwind,
    });
    let suspended = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registered_suspend.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Return {
            value: Some(suspended_value(outer_step)),
        },
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, unwind);
    let claim_success_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.claim_success_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: success,
            else_block: invalid,
        },
        unwind,
    });
    let claim_failure_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.claim_failure_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: failure,
            else_block: invalid,
        },
        unwind,
    });
    let check_latched_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_latched_failure.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_LATCHED_FAILURE,
            ),
            then_block: claim_failure_frame,
            else_block: invalid,
        },
        unwind,
    });
    let check_latched_success = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_latched_success.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_LATCHED_SUCCESS,
            ),
            then_block: claim_success_frame,
            else_block: check_latched_failure,
        },
        unwind,
    });
    let completion_spin = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completion_backoff.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let check_completing_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.check_completing_failure.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_FAILURE,
            ),
            then_block: completion_spin,
            else_block: check_latched_success,
        },
        unwind,
    });
    let completion_wait = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completion_wait.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_SUCCESS,
            ),
            then_block: completion_spin,
            else_block: check_completing_failure,
        },
        unwind,
    });
    body.blocks[completion_spin].terminator = mir::Terminator::Goto(completion_wait);
    let register_return = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.register_return.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: registration_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                ADAPTER_REGISTERING,
                ADAPTER_WAITING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(registration_claim, mir::Type::Int),
                ADAPTER_REGISTERING,
            ),
            then_block: suspended,
            else_block: completion_wait,
        },
        unwind,
    });
    let registration_propagate = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_propagate.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, throwable.clone()),
            unwind,
        },
        unwind,
    });
    let registration_claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: registration_propagate,
            else_block: invalid,
        },
        unwind,
    });
    let registration_protocol = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_protocol.{}", site.state),
        statements: vec![atomic_field_store(
            mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
            1,
            mir::Expr::int(ADAPTER_CONSUMED),
        )],
        terminator: mir::Terminator::Goto(invalid),
        unwind,
    });
    let registration_protocol_claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_protocol_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                i64::from(site.state),
                STATE_RUNNING,
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(frame_claim, mir::Type::Int),
                i64::from(site.state),
            ),
            then_block: registration_protocol,
            else_block: invalid,
        },
        unwind,
    });
    let registration_spin = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_backoff.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let registration_check_completing_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!(
            "coroutine.registration_check_completing_failure.{}",
            site.state
        ),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_FAILURE,
            ),
            then_block: registration_spin,
            else_block: registration_protocol_claim_frame,
        },
        unwind,
    });
    let registration_wait = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_wait.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                atomic_field_load(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                ),
                ADAPTER_COMPLETING_SUCCESS,
            ),
            then_block: registration_spin,
            else_block: registration_check_completing_failure,
        },
        unwind,
    });
    body.blocks[registration_spin].terminator = mir::Terminator::Goto(registration_wait);
    let registration_failure = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.registration_failure.{}", site.state),
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
            statement(mir::StatementKind::ValDecl {
                local: registration_claim,
                init: atomic_field_compare_exchange(
                    mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                    1,
                    ADAPTER_REGISTERING,
                    ADAPTER_CONSUMED,
                ),
            }),
        ],
        terminator: mir::Terminator::Branch {
            cond: int_eq(
                mir::Expr::local(registration_claim, mir::Type::Int),
                ADAPTER_REGISTERING,
            ),
            then_block: registration_claim_frame,
            else_block: registration_wait,
        },
        unwind: None,
    });
    body.blocks[register_call].terminator = mir::Terminator::Goto(register_return);
    body.blocks[register_call].unwind = Some(registration_failure);

    let mut resume_statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            resume_statements.push(restore_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                *local,
                slot,
            ));
        }
    }
    if let Some(destination) = site.destination {
        resume_statements.push(restore_statement(
            mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
            destination,
            &frame_slots[&destination],
        ));
    }
    let resume_block = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.resume.{}", site.state),
        statements: resume_statements,
        terminator: mir::Terminator::Goto(post),
        unwind,
    });
    GeneratedSite {
        state: i64::from(site.state),
        resume_block,
        failure_state: failure_state(site.state),
        failure_block: failure_resume_block(
            body,
            frame_local,
            frame_slots,
            failure_slot,
            &site,
            unwind,
        ),
        point: adapter.point,
    }
}
