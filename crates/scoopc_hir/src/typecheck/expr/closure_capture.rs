//! Closure capture rules that must be enforced before HIR/codegen.

use std::collections::HashSet;

use crate::ast;
use crate::span::Span;

use super::ExprTypeError;

struct CapturedVarUse {
    name: String,
    span: Span,
}

/// Rejects references from one lambda body to `var` bindings declared outside it.
pub(super) fn reject_lambda_var_captures(
    lam: &ast::LambdaExpr,
    outer_mutable_bindings: Option<&HashSet<Span>>,
) -> Result<(), ExprTypeError> {
    let Some(outer_mutable_bindings) = outer_mutable_bindings else {
        return Ok(());
    };
    if outer_mutable_bindings.is_empty() {
        return Ok(());
    }

    if let Some(capture) = find_captured_var_use(lam.body.as_ref(), outer_mutable_bindings) {
        return Err(ExprTypeError::ClosureVarCaptureNotAllowed {
            name: capture.name,
            span: capture.span.into(),
        });
    }
    Ok(())
}

/// Finds the first outer `var` use in this lambda body, skipping nested lambdas.
fn find_captured_var_use(
    expr: &ast::Expr,
    outer_mutable_bindings: &HashSet<Span>,
) -> Option<CapturedVarUse> {
    match &expr.kind {
        ast::ExprKind::Missing
        | ast::ExprKind::IntLit
        | ast::ExprKind::FloatLit
        | ast::ExprKind::CharLit
        | ast::ExprKind::StringLit
        | ast::ExprKind::UnitLit
        | ast::ExprKind::ClassLit { .. } => None,
        ast::ExprKind::Annotated { expr, .. } => {
            find_captured_var_use(expr, outer_mutable_bindings)
        }
        ast::ExprKind::Ident(id) => match id.resolved.as_ref() {
            Some(ast::ResolvedValueRef::Local { name, decl_span })
                if outer_mutable_bindings.contains(decl_span) =>
            {
                Some(CapturedVarUse {
                    name: name.clone(),
                    span: id.span,
                })
            }
            _ => None,
        },
        ast::ExprKind::TupleLit { elements } | ast::ExprKind::ArrayLit { elements } => {
            find_captured_var_use_in_exprs(elements, outer_mutable_bindings)
        }
        ast::ExprKind::InterpolatedString { parts, .. } => {
            parts.iter().find_map(|part| match part {
                ast::InterpolatedStringPart::Text { .. } => None,
                ast::InterpolatedStringPart::Expr { expr } => {
                    find_captured_var_use(expr, outer_mutable_bindings)
                }
            })
        }
        ast::ExprKind::Block(block)
        | ast::ExprKind::DoBlock { body: block, .. }
        | ast::ExprKind::UnsafeBlock { body: block, .. }
        | ast::ExprKind::SafeBlock { body: block, .. } => {
            find_captured_var_use_in_block(block, outer_mutable_bindings)
        }
        ast::ExprKind::Lambda(_) => None,
        ast::ExprKind::StructLit { fields, .. } => fields
            .iter()
            .find_map(|field| find_captured_var_use(&field.value, outer_mutable_bindings)),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => find_captured_var_use(cond, outer_mutable_bindings)
            .or_else(|| find_captured_var_use(then_branch, outer_mutable_bindings))
            .or_else(|| {
                else_branch
                    .as_deref()
                    .and_then(|expr| find_captured_var_use(expr, outer_mutable_bindings))
            }),
        ast::ExprKind::When { subject, arms } => {
            find_captured_var_use(subject, outer_mutable_bindings).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(|guard| find_captured_var_use(guard, outer_mutable_bindings))
                        .or_else(|| find_captured_var_use(&arm.body, outer_mutable_bindings))
                })
            })
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => find_captured_var_use_in_block(body, outer_mutable_bindings)
            .or_else(|| {
                arms.iter()
                    .find_map(|arm| find_captured_var_use(&arm.body, outer_mutable_bindings))
            })
            .or_else(|| {
                finally
                    .as_ref()
                    .and_then(|block| find_captured_var_use_in_block(block, outer_mutable_bindings))
            }),
        ast::ExprKind::MemberAccess { receiver, .. }
        | ast::ExprKind::SafeMemberAccess { receiver, .. }
        | ast::ExprKind::TypeApply {
            callee: receiver, ..
        }
        | ast::ExprKind::NotNullAssert { expr: receiver, .. }
        | ast::ExprKind::SpreadArg { expr: receiver, .. }
        | ast::ExprKind::TypeCheck { expr: receiver, .. }
        | ast::ExprKind::Cast { expr: receiver, .. } => {
            find_captured_var_use(receiver, outer_mutable_bindings)
        }
        ast::ExprKind::SpliceField { receiver, field } => {
            find_captured_var_use(receiver, outer_mutable_bindings)
                .or_else(|| find_captured_var_use(field, outer_mutable_bindings))
        }
        ast::ExprKind::Call { callee, args } => {
            find_captured_var_use(callee, outer_mutable_bindings)
                .or_else(|| find_captured_var_use_in_exprs(args, outer_mutable_bindings))
        }
        ast::ExprKind::NamedArg { value, .. } => {
            find_captured_var_use(value, outer_mutable_bindings)
        }
        ast::ExprKind::Unary { expr, .. } => find_captured_var_use(expr, outer_mutable_bindings),
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            find_captured_var_use(lhs, outer_mutable_bindings)
                .or_else(|| find_captured_var_use(rhs, outer_mutable_bindings))
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => {
            find_captured_var_use(lhs, outer_mutable_bindings)
                .or_else(|| find_captured_var_use(rhs, outer_mutable_bindings))
        }
        ast::ExprKind::WithUpdate { base, updates, .. } => {
            find_captured_var_use(base, outer_mutable_bindings).or_else(|| {
                updates
                    .iter()
                    .find_map(|update| find_captured_var_use(&update.value, outer_mutable_bindings))
            })
        }
    }
}

/// Scans a sequence of expressions in source order.
fn find_captured_var_use_in_exprs(
    exprs: &[ast::Expr],
    outer_mutable_bindings: &HashSet<Span>,
) -> Option<CapturedVarUse> {
    exprs
        .iter()
        .find_map(|expr| find_captured_var_use(expr, outer_mutable_bindings))
}

/// Scans block statements without treating local declarations as captures.
fn find_captured_var_use_in_block(
    block: &ast::Block,
    outer_mutable_bindings: &HashSet<Span>,
) -> Option<CapturedVarUse> {
    block.stmts.iter().find_map(|stmt| match &stmt.kind {
        ast::StmtKind::Empty | ast::StmtKind::Break { .. } | ast::StmtKind::Continue { .. } => None,
        ast::StmtKind::Expr(expr) => find_captured_var_use(expr, outer_mutable_bindings),
        ast::StmtKind::Val(val) => val
            .init
            .as_ref()
            .and_then(|init| find_captured_var_use(init, outer_mutable_bindings)),
        ast::StmtKind::Return { value, .. } => value
            .as_ref()
            .and_then(|value| find_captured_var_use(value, outer_mutable_bindings)),
        ast::StmtKind::While { cond, body, .. } => {
            find_captured_var_use(cond, outer_mutable_bindings)
                .or_else(|| find_captured_var_use_in_block(body, outer_mutable_bindings))
        }
        ast::StmtKind::For(for_stmt) => {
            find_captured_var_use(&for_stmt.iter, outer_mutable_bindings)
                .or_else(|| find_captured_var_use_in_block(&for_stmt.body, outer_mutable_bindings))
        }
        ast::StmtKind::Missing => None,
    })
}
