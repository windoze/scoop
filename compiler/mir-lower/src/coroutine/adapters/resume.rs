//! Successful continuation callback generation.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_resume_method(
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
    state: mir::CoroutineSuspendStateId,
    result: &mir::Type,
    latch: Option<FrameSlot>,
) -> mir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Class(adapter)));
    let value = locals.alloc(local("value", result.clone()));
    let step = locals.alloc(local("$step", outer_step.clone()));
    let adapter_claim = locals.alloc(local(
        "$adapter_claim",
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState),
    ));
    let frame_claim = locals.alloc(local(
        "$frame_claim",
        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
    ));
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
                adapter_state(ADAPTER_CONSUMED),
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
                            frame_state(suspended_state(state)),
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
            adapter_state(ADAPTER_CONSUMED),
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
                frame_state_value(suspended_state(state)),
                frame_state_value(STATE_RUNNING),
            ),
        })],
        terminator: mir::Terminator::Branch {
            cond: machine_eq(
                mir::Expr::local(
                    frame_claim,
                    mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
                ),
                frame_state_value(suspended_state(state)),
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
                adapter_state_value(ADAPTER_WAITING),
                adapter_state_value(ADAPTER_COMPLETING_SUCCESS),
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
                    adapter_state(ADAPTER_LATCHED_SUCCESS),
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
                    adapter_state_value(ADAPTER_REGISTERING),
                    adapter_state_value(ADAPTER_COMPLETING_SUCCESS),
                ),
            })],
            terminator: mir::Terminator::Branch {
                cond: machine_eq(
                    mir::Expr::local(
                        adapter_claim,
                        mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState),
                    ),
                    adapter_state_value(ADAPTER_REGISTERING),
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
