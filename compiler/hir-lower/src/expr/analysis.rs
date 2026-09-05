use super::*;

/// Collect the `x is T` facts established by `cond` evaluating to
/// `outcome` (see `resolve_smart_casts`).
pub(super) fn collect_smart_cast_candidates<'a>(
    cond: &'a ast::Expr,
    outcome: bool,
    out: &mut Vec<(&'a ast::Ident, &'a ast::TypeRef)>,
) {
    match cond {
        // `x is T` holds exactly when the check is not negated and the
        // condition is true (or it is negated and the condition is
        // false).
        ast::Expr::Is {
            operand,
            ty,
            negated,
            ..
        } => {
            if outcome == !negated {
                if let ast::Expr::Var(name) = &**operand {
                    out.push((name, ty));
                }
            }
        }
        ast::Expr::Unary {
            op: ast::UnOp::Not,
            operand,
            ..
        } => collect_smart_cast_candidates(operand, !outcome, out),
        // `a && b` is true only when both hold; a false conjunction
        // establishes nothing (M6: no `||` support).
        ast::Expr::Binary {
            op: ast::BinOp::And,
            lhs,
            rhs,
            ..
        } if outcome => {
            collect_smart_cast_candidates(lhs, true, out);
            collect_smart_cast_candidates(rhs, true, out);
        }
        _ => {}
    }
}

pub(super) fn block_contains_return(block: &ast::Block) -> bool {
    block.statements.iter().any(statement_contains_return)
}

fn statement_contains_return(statement: &ast::Statement) -> bool {
    match &statement.kind {
        ast::StatementKind::Return { .. } => true,
        ast::StatementKind::LocalFunction(_) => false,
        ast::StatementKind::Expr(expr) | ast::StatementKind::Throw(expr) => {
            expr_contains_return(expr)
        }
        ast::StatementKind::ValDecl(decl) => expr_contains_return(&decl.init),
        ast::StatementKind::LocalDelegatedProperty(decl) => expr_contains_return(&decl.expression),
        ast::StatementKind::Assign(assign) => expr_contains_return(&assign.value),
        ast::StatementKind::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::While(while_) => {
            expr_contains_return(&while_.cond) || block_contains_return(&while_.body)
        }
        ast::StatementKind::Block(block) | ast::StatementKind::SafetyBlock { block, .. } => {
            block_contains_return(block)
        }
        ast::StatementKind::When(when) => {
            expr_contains_return(&when.subject)
                || when.arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
    }
}

fn expr_contains_return(expr: &ast::Expr) -> bool {
    match expr {
        // A nested callable owns its own return target.
        ast::Expr::Lambda { .. }
        | ast::Expr::AnonymousFunction { .. }
        | ast::Expr::CallableReference { .. } => false,
        ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
            elements.iter().any(expr_contains_return)
        }
        ast::Expr::StructInit { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::FieldAccess(access) => expr_contains_return(&access.receiver),
        ast::Expr::Call(call) => call
            .args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::Invoke { callee, args, .. } => {
            expr_contains_return(callee)
                || args
                    .iter()
                    .any(|argument| expr_contains_return(&argument.expression))
        }
        ast::Expr::InfixCall { lhs, rhs, .. }
        | ast::Expr::Binary { lhs, rhs, .. }
        | ast::Expr::Elvis { lhs, rhs, .. } => {
            expr_contains_return(lhs) || expr_contains_return(rhs)
        }
        ast::Expr::Unary { operand, .. }
        | ast::Expr::NullAssert { operand, .. }
        | ast::Expr::Is { operand, .. }
        | ast::Expr::Cast { operand, .. } => expr_contains_return(operand),
        ast::Expr::MethodCall { receiver, args, .. } => {
            expr_contains_return(receiver)
                || args
                    .iter()
                    .any(|argument| expr_contains_return(&argument.expression))
        }
        ast::Expr::SuperMethodCall { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::QualifiedInterfaceSuperMethodCall { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::Index {
            receiver, indices, ..
        } => expr_contains_return(receiver) || indices.iter().any(expr_contains_return),
        ast::Expr::Update { place, .. } => place_contains_return(place),
        ast::Expr::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::When(when) => {
            expr_contains_return(&when.subject)
                || when.arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
        ast::Expr::StringLiteral { .. }
        | ast::Expr::IntLiteral(_)
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::UnitLiteral { .. }
        | ast::Expr::Var(_)
        | ast::Expr::This { .. }
        | ast::Expr::QualifiedInterfaceSuperAccess { .. } => false,
    }
}

fn place_contains_return(place: &ast::PlaceExpr) -> bool {
    match place {
        ast::PlaceExpr::Name(_) => false,
        ast::PlaceExpr::Field { receiver, .. } => expr_contains_return(receiver),
        ast::PlaceExpr::Index {
            receiver, indices, ..
        } => expr_contains_return(receiver) || indices.iter().any(expr_contains_return),
        ast::PlaceExpr::QualifiedInterfaceSuperProperty { .. } => false,
    }
}
