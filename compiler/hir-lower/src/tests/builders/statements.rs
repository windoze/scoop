use super::*;

// --- statements ---

pub(crate) fn stmt(expr: Expr) -> Statement {
    Statement {
        kind: StatementKind::Expr(expr),
        span: sp(),
    }
}

pub(crate) fn ret(value: Option<Expr>) -> Statement {
    Statement {
        kind: StatementKind::Return { value },
        span: sp(),
    }
}

pub(crate) fn val(name: &str, init: Expr) -> Statement {
    val_ty(name, None, init)
}

pub(crate) fn var_(name: &str, init: Expr) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable: true,
            target: pat_bind(name),
            ty: None,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn val_ty(name: &str, ty: Option<TypeRef>, init: Expr) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable: false,
            target: pat_bind(name),
            ty,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

/// A destructuring `val` / `var` declaration (spec 4.6).
pub(crate) fn val_pat(
    mutable: bool,
    target: ast::Pattern,
    ty: Option<TypeRef>,
    init: Expr,
) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable,
            target,
            ty,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn when_stmt(
    subject: Expr,
    arms: Vec<ast::WhenArm>,
    else_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::When(ast::When {
            subject: ast::WhenSubject::Expression(subject),
            arms,
            else_body: else_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn arm(
    pattern: ast::Pattern,
    guard: Option<Expr>,
    body: Vec<Statement>,
) -> ast::WhenArm {
    ast::WhenArm {
        condition: ast::WhenArmCondition::Case(pattern),
        guard,
        body: block(body),
        span: sp(),
    }
}

pub(crate) fn assign(target: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Name(ident(target)),
            op: ast::AssignmentOp::Assign,
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

/// `receiver.field = value` (M6).
pub(crate) fn assign_field(receiver: Expr, name: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Field {
                receiver: Box::new(receiver),
                name: ident(name),
                span: sp(),
            },
            op: ast::AssignmentOp::Assign,
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

/// `receiver[index] = value` (M5).
pub(crate) fn assign_index(receiver: Expr, index: Expr, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Index {
                receiver: Box::new(receiver),
                indices: ast::NonEmptyVec::new(index, Vec::new()),
                span: sp(),
            },
            op: ast::AssignmentOp::Assign,
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn if_stmt(
    cond: Expr,
    then_body: Vec<Statement>,
    else_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::If(ast::If {
            cond,
            then_block: block(then_body),
            else_block: else_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn while_stmt(cond: Expr, body: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::While(ast::While {
            cond,
            body: block(body),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn block_stmt(statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::Block(block(statements)),
        span: sp(),
    }
}

pub(crate) fn unsafe_block(statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::SafetyBlock {
            mode: ast::SafetyMode::Unsafe,
            block: block(statements),
        },
        span: sp(),
    }
}

pub(crate) fn block(statements: Vec<Statement>) -> Block {
    Block {
        statements,
        span: sp(),
    }
}
