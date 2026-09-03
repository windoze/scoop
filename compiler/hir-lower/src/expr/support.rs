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
