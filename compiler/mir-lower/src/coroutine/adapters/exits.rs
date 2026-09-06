//! Shared completion, suspension, and failure exits for adapter callbacks.

use super::*;

pub(super) struct DriveExitBlocks {
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
    step_ty: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
) -> DriveExitBlocks {
    let completed_payload = lowerer
        .coroutines
        .step_metadata_for_type(step_ty)
        .completed_payload();
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
                frame_state(STATE_COMPLETED),
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
                            variant: completed_payload.variant().variant_index(),
                            index: completed_payload.field_index(),
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
                frame_state(STATE_COMPLETED),
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
