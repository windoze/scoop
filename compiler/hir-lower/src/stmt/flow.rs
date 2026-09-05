use super::*;

/// Whether control can reach the end of a statement list. Once one
/// statement cannot fall through, later statements are unreachable
/// and cannot make the list fall through again.
pub(crate) fn statements_can_fall_through(statements: &[hir::Statement]) -> bool {
    statements.iter().all(statement_can_fall_through)
}

/// Structured fallthrough analysis for the non-Unit return rule.
/// `when` is exhaustive by the time HIR exists; an empty arm list is
/// kept conservative for malformed HIR produced after diagnostics.
fn statement_can_fall_through(statement: &hir::Statement) -> bool {
    match &statement.kind {
        hir::StatementKind::Return { .. } | hir::StatementKind::Throw(_) => false,
        hir::StatementKind::If {
            then_body,
            else_body,
            ..
        } => {
            statements_can_fall_through(then_body)
                || else_body.as_deref().is_none_or(statements_can_fall_through)
        }
        hir::StatementKind::When(when) => {
            when.arms
                .iter()
                .any(|arm| statements_can_fall_through(&arm.body))
                || match &when.fallback {
                    hir::WhenFallback::Else(body) => statements_can_fall_through(body),
                    hir::WhenFallback::Impossible(_) => false,
                }
        }
        hir::StatementKind::Try(try_) => {
            if try_
                .finally_body
                .as_deref()
                .is_some_and(|body| !statements_can_fall_through(body))
            {
                return false;
            }
            statements_can_fall_through(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| statements_can_fall_through(&catch.body))
        }
        hir::StatementKind::Expr(_)
        | hir::StatementKind::InitializationEnsure(_)
        | hir::StatementKind::LocalFunction(_)
        | hir::StatementKind::ValDecl { .. }
        | hir::StatementKind::Assign { .. }
        | hir::StatementKind::While { .. } => true,
    }
}
