//! Shared completion, suspension, and failure exits for adapter callbacks.

use super::*;

pub(super) struct DriveExitBlocks {
    pub(super) switch: crate::task_context::TaskSwitch,
    pub(super) completed: mir::BlockId,
    pub(super) suspended: mir::BlockId,
    pub(super) catch_pad: mir::BlockId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive_exit_blocks(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    this: mir::LocalId,
    step: mir::LocalId,
    adapter: mir::ClassId,
    frame_class: mir::ClassId,
    frame_layout: FrameLayout,
    step_ty: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: &mir::CallTarget,
    outer_failure: &mir::CallTarget,
) -> DriveExitBlocks {
    let switch =
        crate::task_context::TaskSwitch::new(frame_layout.task_storage.core, locals, blocks);
    let completed_payload = lowerer
        .coroutines
        .step_metadata_for_type(step_ty)
        .completed_payload();
    let throwable = crate::coroutine_registry::throwable_type(module, &lowerer.class_map);
    let exception = locals.alloc(local("$uncaught", throwable.clone()));
    let completion_ty = mir::Type::Interface(outer_continuation);
    let completed_value_ty = lowerer
        .coroutines
        .step_metadata_for_type(step_ty)
        .result()
        .clone();
    let completed = blocks.alloc(mir::BasicBlock {
        name: "completed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                frame_layout.state.field_index(),
                frame_state(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: outer_resume.clone(),
                args: vec![
                    frame_field(
                        adapter_frame(this, adapter, frame_class),
                        frame_layout.completion.field_index(),
                        completion_ty.clone(),
                    ),
                    mir::Expr::new(
                        completed_value_ty,
                        mir::ExprKind::EnumField {
                            operand: Box::new(mir::Expr::local(step, step_ty.clone())),
                            variant: completed_payload.variant().variant_index(),
                            index: completed_payload.field_index(),
                        },
                    ),
                ],
                pending: mir::CoroutinePendingContext::Root,
            }))),
        ],
        terminator: mir::Terminator::Return { value: None },
        unwind: Some(switch.unwind),
    });
    let suspended = blocks.alloc(mir::BasicBlock {
        name: "suspended".to_string(),
        statements: vec![switch.leave()],
        terminator: mir::Terminator::Return { value: None },
        unwind: None,
    });
    let failed = blocks.alloc(mir::BasicBlock {
        name: "failed".to_string(),
        statements: vec![
            atomic_field_store(
                adapter_frame(this, adapter, frame_class),
                frame_layout.state.field_index(),
                frame_state(STATE_COMPLETED),
            ),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: outer_failure.clone(),
                args: vec![
                    frame_field(
                        adapter_frame(this, adapter, frame_class),
                        frame_layout.completion.field_index(),
                        completion_ty,
                    ),
                    mir::Expr::local(exception, throwable.clone()),
                ],
                pending: mir::CoroutinePendingContext::Root,
            }))),
        ],
        terminator: mir::Terminator::Return { value: None },
        unwind: Some(switch.unwind),
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
        unwind: Some(switch.catch_unwind),
    });
    blocks[completed].statements.push(switch.leave());
    blocks[failed].statements.push(switch.leave());
    DriveExitBlocks {
        switch,
        completed,
        suspended,
        catch_pad,
    }
}
