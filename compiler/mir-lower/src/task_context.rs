//! Task entry and cleanup shared by coroutine and callback drivers.

use super::*;

pub(super) struct TaskSwitch {
    guard: mir::LocalId,
    storage: mir::ContextStorageType,
    pub(super) unwind: mir::BlockId,
    pub(super) catch_unwind: mir::BlockId,
}

impl TaskSwitch {
    pub(super) fn new(
        core: mir::ConeIdentity,
        locals: &mut Arena<mir::Local>,
        blocks: &mut Arena<mir::BasicBlock>,
    ) -> Self {
        let storage = mir::ContextStorageType::new(core, mir::ContextStorageRole::SwitchGuard);
        let guard = locals.alloc(mir::Local {
            name: "$context_guard".to_string(),
            ty: mir::Type::Context(storage),
            mutable: false,
        });
        let unwind = cleanup(blocks, vec![leave(guard, storage)], "context.unwind");
        let catch_unwind = cleanup(
            blocks,
            vec![
                stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
                leave(guard, storage),
            ],
            "context.catch_unwind",
        );
        Self {
            guard,
            storage,
            unwind,
            catch_unwind,
        }
    }

    pub(super) fn enter(&self, task: mir::Expr) -> mir::Statement {
        stmt(mir::StatementKind::ValDecl {
            local: self.guard,
            init: mir::Expr::new(
                mir::Type::Context(self.storage),
                mir::ExprKind::Context(mir::ContextOperation::Enter {
                    task: Box::new(task),
                }),
            ),
        })
    }

    pub(super) fn leave(&self) -> mir::Statement {
        leave(self.guard, self.storage)
    }
}

pub(super) fn fork_current(
    core: mir::ConeIdentity,
    locals: &mut Arena<mir::Local>,
) -> (mir::Statement, mir::Expr) {
    let root = mir::Expr::new(
        mir::Type::Context(mir::ContextStorageType::new(
            core,
            mir::ContextStorageRole::Node,
        )),
        mir::ExprKind::Context(mir::ContextOperation::Snapshot),
    );
    fork(core, root, locals)
}

pub(super) fn fork(
    core: mir::ConeIdentity,
    root: mir::Expr,
    locals: &mut Arena<mir::Local>,
) -> (mir::Statement, mir::Expr) {
    let ty = mir::Type::Context(mir::ContextStorageType::new(
        core,
        mir::ContextStorageRole::Task,
    ));
    let child = locals.alloc(mir::Local {
        name: "$child_context".to_string(),
        ty: ty.clone(),
        mutable: false,
    });
    let statement = stmt(mir::StatementKind::ValDecl {
        local: child,
        init: mir::Expr::new(
            ty.clone(),
            mir::ExprKind::Context(mir::ContextOperation::Fork {
                root: Box::new(root),
            }),
        ),
    });
    (statement, mir::Expr::local(child, ty))
}

fn leave(guard: mir::LocalId, storage: mir::ContextStorageType) -> mir::Statement {
    stmt(mir::StatementKind::Expr(mir::Expr::new(
        mir::Type::Unit,
        mir::ExprKind::Context(mir::ContextOperation::Leave {
            guard: Box::new(mir::Expr::local(guard, mir::Type::Context(storage))),
        }),
    )))
}

fn cleanup(
    blocks: &mut Arena<mir::BasicBlock>,
    mut statements: Vec<mir::Statement>,
    name: &str,
) -> mir::BlockId {
    statements.insert(
        0,
        stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        })),
    );
    blocks.alloc(mir::BasicBlock {
        name: name.to_string(),
        statements,
        terminator: mir::Terminator::Resume,
        unwind: None,
    })
}

fn stmt(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement {
        kind,
        span: mir::SourceSpan::new(0, 0).expect("the generated span is ordered"),
    }
}
