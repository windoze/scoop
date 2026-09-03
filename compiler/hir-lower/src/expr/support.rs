use super::*;

/// The else half of a `?.` / `?:` desugaring: the statements evaluating
/// the fallback (lazily, inside the branch), then the fallback value.
pub(super) struct ElseBranch {
    pub(super) statements: Vec<hir::Statement>,
    pub(super) value: hir::Expr,
}

/// Whether the expression is the `None` construction (see
/// `lower_binary`).
pub(super) fn is_none_literal(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Var(name) if name.text == "None")
}

/// Copy a variant field default. Defaults are literals
/// (`resolve_variant_default` enforces this), so copying is trivial.
pub(super) fn clone_literal(expr: &hir::Expr) -> hir::Expr {
    let kind = match &expr.kind {
        ExprKind::IntLiteral(value) => ExprKind::IntLiteral(*value),
        ExprKind::StringLiteral(value) => ExprKind::StringLiteral(value.clone()),
        ExprKind::BoolLiteral(value) => ExprKind::BoolLiteral(*value),
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op: *op,
            operand: Box::new(clone_literal(operand)),
        },
        _ => unreachable!("variant defaults are literals (resolve_variant_default)"),
    };
    hir::Expr {
        kind,
        ty: expr.ty,
        span: expr.span,
    }
}

pub(super) fn convert_bin_op(op: ast::BinOp) -> (hir::BinOp, &'static str) {
    match op {
        ast::BinOp::Add => (hir::BinOp::Add, "+"),
        ast::BinOp::Sub => (hir::BinOp::Sub, "-"),
        ast::BinOp::Mul => (hir::BinOp::Mul, "*"),
        ast::BinOp::Div => (hir::BinOp::Div, "/"),
        ast::BinOp::Lt => (hir::BinOp::Lt, "<"),
        ast::BinOp::Le => (hir::BinOp::Le, "<="),
        ast::BinOp::Gt => (hir::BinOp::Gt, ">"),
        ast::BinOp::Ge => (hir::BinOp::Ge, ">="),
        ast::BinOp::Eq => (hir::BinOp::Eq, "=="),
        ast::BinOp::Ne => (hir::BinOp::Ne, "!="),
        // Intercepted by `lower_ref_eq` before `convert_bin_op` is
        // reached; mapped here for completeness.
        ast::BinOp::RefEq => (hir::BinOp::RefEq, "==="),
        ast::BinOp::RefNe => (hir::BinOp::RefNe, "!=="),
        ast::BinOp::And => (hir::BinOp::And, "&&"),
        ast::BinOp::Or => (hir::BinOp::Or, "||"),
    }
}
