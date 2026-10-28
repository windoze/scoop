//! Managed invocation body of one foreign callback application.

use super::*;

mod storage;

pub(super) fn build(
    core: mir::ConeIdentity,
    managed_signature: mir::FunctionTypeId,
    signature: &mir::FunctionType,
    throwable: mir::Type,
    index: usize,
    span: mir::SourceSpan,
) -> mir::Function {
    let mut locals = Arena::new();
    let storage = storage::Storage::new(
        core,
        managed_signature,
        signature,
        throwable.clone(),
        &mut locals,
    );
    let exception = locals.alloc(mir::Local {
        debug_origin: Default::default(),
        name: "$caught".to_string(),
        ty: throwable.clone(),
        mutable: false,
    });
    let statement = |kind| mir::Statement {
        debug_origin: Default::default(),
        kind,
        span,
    };
    let mut blocks = Arena::new();
    let switch = crate::task_context::TaskSwitch::new(core, &mut locals, &mut blocks);
    let catch = blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "callback.failure".to_string(),
        statements: vec![
            statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            })),
            statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::new(throwable.clone(), mir::ExprKind::CaughtException),
            }),
            statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            statement(mir::StatementKind::Expr(store(
                storage.exception.clone(),
                mir::Expr::local(exception, throwable),
            ))),
            switch.leave(),
        ],
        terminator: status(mir::ForeignCallbackStatus::Threw),
        unwind: Some(switch.catch_unwind),
    });

    let call = mir::Call {
        target: mir::CallTarget::Closure {
            function_type: managed_signature,
            receiver: Box::new(storage.closure.clone()),
        },
        args: storage.call_arguments(signature),
    };
    let mut success = Vec::new();
    if signature.return_type == mir::Type::Unit {
        success.push(statement(mir::StatementKind::Call(mir::CallEffect::Unit(
            call,
        ))));
    } else {
        let value = locals.alloc(mir::Local {
            debug_origin: Default::default(),
            name: "$value".to_string(),
            ty: signature.return_type.clone(),
            mutable: false,
        });
        success.push(statement(mir::StatementKind::Call(
            mir::CallEffect::Value {
                destination: value,
                call,
            },
        )));
        success.push(statement(mir::StatementKind::Expr(store(
            storage.result.clone(),
            mir::Expr::local(value, signature.return_type.clone()),
        ))));
    }
    success.push(switch.leave());
    let invoke = blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "callback.invoke".to_string(),
        statements: success,
        terminator: status(mir::ForeignCallbackStatus::Returned),
        unwind: Some(catch),
    });
    let (fork, task) = crate::task_context::fork(core, storage.snapshot, &mut locals);
    let entry = blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        statements: vec![fork, switch.enter(task)],
        terminator: mir::Terminator::Goto(invoke),
        unwind: None,
    });
    mir::Function {
        debug_origin: Default::default(),
        gc_effect: mir::GcEffect::Managed,
        name: format!("foreign callback adapter {index}"),
        params: storage.params,
        return_ty: mir::Type::MachineScalar(mir::MachineScalarKind::ForeignCallbackStatus),
        body: mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    }
}

fn store(pointer: mir::Expr, value: mir::Expr) -> mir::Expr {
    mir::Expr::new(
        mir::Type::Unit,
        mir::ExprKind::PtrStore {
            pointer: Box::new(pointer),
            pointee: Box::new(value.ty.clone()),
            offset: None,
            value: Box::new(value),
        },
    )
}

fn status(status: mir::ForeignCallbackStatus) -> mir::Terminator {
    mir::Terminator::Return {
        value: mir::Expr::machine_scalar(mir::MachineScalarValue::ForeignCallbackStatus(status)),
    }
}
