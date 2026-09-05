use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn rewrite_site(
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
) -> GeneratedSite {
    let block = &mut body.blocks[site.block];
    let suffix = block.statements.split_off(site.statement + 1);
    let suspend_statement = block
        .statements
        .pop()
        .expect("suspend site statement exists");
    let terminator = std::mem::replace(&mut block.terminator, mir::Terminator::Unreachable);
    let unwind = block.unwind;
    let post = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.post.{}", site.state),
        statements: suffix,
        terminator,
        unwind,
    });

    let (mut call, destination) = match suspend_statement.kind {
        mir::StatementKind::Call(mir::CallEffect::Unit(call)) => (call, None),
        mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) => {
            (call, Some(destination))
        }
        _ => unreachable!("suspend site is an explicit call effect"),
    };
    assert_eq!(destination, site.destination);
    let SuspendKind::Call = site.kind else {
        return rewrite_intrinsic_site(
            lowerer,
            module,
            body,
            frame_local,
            frame_class,
            frame,
            frame_slots,
            failure_slot.clone(),
            outer_step,
            outer_continuation,
            outer_resume,
            outer_failure,
            source_symbol,
            driver,
            site,
            call,
            post,
            unwind,
        );
    };

    let adapter = generate_adapter(
        lowerer,
        module,
        frame_class,
        frame,
        destination.map(|local| frame_slots[&local].clone()),
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
        None,
    );
    let adapter_local = body.locals.alloc(mir::Local {
        name: format!("$adapter.{}", site.state),
        ty: mir::Type::Class(adapter.class),
        mutable: false,
    });
    let step_ty = callee_return_type(lowerer, call.target.callee);
    let step_local = body.locals.alloc(mir::Local {
        name: format!("$step.{}", site.state),
        ty: step_ty.clone(),
        mutable: false,
    });

    let block = &mut body.blocks[site.block];
    for local in site
        .live_after
        .iter()
        .copied()
        .filter(|local| Some(*local) != destination)
    {
        if let Some(slot) = frame_slots.get(&local) {
            block.statements.push(save_statement(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                mir::Expr::local(local, body.locals[local].ty.clone()),
                slot,
            ));
        }
    }
    block.statements.push(atomic_field_store(
        mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
        0,
        frame_state(suspended_state(site.state)),
    ));
    block.statements.extend(initialized_generated_class(
        adapter_local,
        adapter.class,
        vec![
            mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
            adapter_state(ADAPTER_WAITING),
        ],
    ));
    call.args.push(mir::Expr::local(
        adapter_local,
        mir::Type::Class(adapter.class),
    ));
    block.statements.push(statement(mir::StatementKind::Call(
        mir::CallEffect::Value {
            destination: step_local,
            call,
        },
    )));

    let completed = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed.{}", site.state),
        statements: {
            let mut statements = Vec::new();
            if let Some(destination) = destination {
                statements.push(statement(mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::new(
                        site.result.clone(),
                        mir::ExprKind::EnumField {
                            operand: Box::new(mir::Expr::local(step_local, step_ty.clone())),
                            variant: 0,
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
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, unwind);
    let adapter_claim = body.locals.alloc(local(
        &format!("$completed_adapter_claim.{}", site.state),
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState),
    ));
    let frame_claim = body.locals.alloc(local(
        &format!("$completed_frame_claim.{}", site.state),
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
    ));
    let claim_frame = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed_claim_frame.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: frame_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                0,
                frame_state_value(suspended_state(site.state)),
                frame_state_value(STATE_RUNNING),
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: machine_eq(
                mir::Expr::local(
                    frame_claim,
                    mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
                ),
                frame_state_value(suspended_state(site.state)),
            ),
            then_block: completed,
            else_block: invalid,
        },
        unwind,
    });
    let claim_completed = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.completed_claim_adapter.{}", site.state),
        statements: vec![statement(mir::StatementKind::ValDecl {
            local: adapter_claim,
            init: atomic_field_compare_exchange(
                mir::Expr::local(adapter_local, mir::Type::Class(adapter.class)),
                1,
                adapter_state_value(ADAPTER_WAITING),
                adapter_state_value(ADAPTER_CONSUMED),
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: machine_eq(
                mir::Expr::local(
                    adapter_claim,
                    mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState),
                ),
                adapter_state_value(ADAPTER_WAITING),
            ),
            then_block: claim_frame,
            else_block: invalid,
        },
        unwind,
    });
    let suspended = body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.suspended.{}", site.state),
        statements: Vec::new(),
        terminator: mir::Terminator::Return {
            value: Some(suspended_value(outer_step)),
        },
        unwind: None,
    });
    body.blocks[site.block].terminator = mir::Terminator::Branch {
        cond: is_completed(step_local, step_ty),
        then_block: claim_completed,
        else_block: suspended,
    };

    let mut resume_statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == destination {
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
    if let Some(destination) = destination {
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
        state: suspended_state(site.state),
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

pub(super) fn failure_resume_block(
    body: &mut mir::Body,
    frame: mir::LocalId,
    frame_slots: &HashMap<mir::LocalId, FrameSlot>,
    failure_slot: FrameSlot,
    site: &SuspendSite,
    unwind: Option<mir::BlockId>,
) -> mir::BlockId {
    let mut statements = Vec::new();
    for local in &site.live_after {
        if Some(*local) == site.destination {
            continue;
        }
        if let Some(slot) = frame_slots.get(local) {
            statements.push(restore_statement(
                mir::Expr::local(frame, body.locals[frame].ty.clone()),
                *local,
                slot,
            ));
        }
    }
    body.blocks.alloc(mir::BasicBlock {
        name: format!("coroutine.failure.{}", site.state),
        statements,
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::new(
                failure_slot.value_ty.clone(),
                mir::ExprKind::EnumField {
                    operand: Box::new(frame_field(
                        mir::Expr::local(frame, body.locals[frame].ty.clone()),
                        failure_slot.field,
                        failure_slot.slot_ty.clone(),
                    )),
                    variant: 1,
                    index: 0,
                },
            ),
            unwind,
        },
        unwind,
    })
}
