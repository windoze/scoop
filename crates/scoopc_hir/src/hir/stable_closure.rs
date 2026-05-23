//! Shared stable closure lexical-path helpers.
//!
//! 这组 helper 为 HIR closure 恢复 `$lambdaN(. $lambdaM)*` 词法路径，供 LLVM private
//! naming 与 RTTI closure-env canonical name 复用同一份 authoritative 规则。

use crate::span::Span;

use super::{Block, CallArg, Expr, ExprKind, FunDecl, InterpolatedStringPart, Stmt, StmtKind};

/// 从顶层/成员 `fun` 的 HIR body 中恢复目标 closure 的稳定词法路径。
pub fn stable_closure_lexical_path_in_fun(fun: &FunDecl, target_span: Span) -> Option<String> {
    let body = fun.body.as_ref()?;
    let mut next_closure_index = 0;
    stable_closure_lexical_path_in_block(body, target_span, None, &mut next_closure_index)
}

/// 从任意表达式根中恢复目标 closure 的稳定词法路径。
pub fn stable_closure_lexical_path_in_expr(expr: &Expr, target_span: Span) -> Option<String> {
    let mut next_closure_index = 0;
    stable_closure_lexical_path_in_expr_impl(expr, target_span, None, &mut next_closure_index)
}

fn stable_closure_lexical_path_in_block(
    block: &Block,
    target_span: Span,
    prefix: Option<&str>,
    next_closure_index: &mut usize,
) -> Option<String> {
    for stmt in &block.stmts {
        if let Some(path) =
            stable_closure_lexical_path_in_stmt(stmt, target_span, prefix, next_closure_index)
        {
            return Some(path);
        }
    }
    None
}

fn stable_closure_lexical_path_in_stmt(
    stmt: &Stmt,
    target_span: Span,
    prefix: Option<&str>,
    next_closure_index: &mut usize,
) -> Option<String> {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => None,
        StmtKind::Expr(expr) => {
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }
        StmtKind::Val(val) => val.init.as_ref().and_then(|expr| {
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }),
        StmtKind::Assign { lhs, rhs, .. } => {
            stable_closure_lexical_path_in_expr_impl(lhs, target_span, prefix, next_closure_index)
                .or_else(|| {
                    stable_closure_lexical_path_in_expr_impl(
                        rhs,
                        target_span,
                        prefix,
                        next_closure_index,
                    )
                })
        }
        StmtKind::While { cond, body } => {
            stable_closure_lexical_path_in_expr_impl(cond, target_span, prefix, next_closure_index)
                .or_else(|| {
                    stable_closure_lexical_path_in_block(
                        body,
                        target_span,
                        prefix,
                        next_closure_index,
                    )
                })
        }
        StmtKind::Return { value } => value.as_ref().and_then(|expr| {
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }),
    }
}

fn stable_closure_lexical_path_in_expr_impl(
    expr: &Expr,
    target_span: Span,
    prefix: Option<&str>,
    next_closure_index: &mut usize,
) -> Option<String> {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Todo(_) => None,
        ExprKind::StructLit { fields, .. } => fields.iter().find_map(|field| {
            stable_closure_lexical_path_in_expr_impl(
                &field.value,
                target_span,
                prefix,
                next_closure_index,
            )
        }),
        ExprKind::TupleLit { elements } => elements.iter().find_map(|element| {
            stable_closure_lexical_path_in_expr_impl(
                element,
                target_span,
                prefix,
                next_closure_index,
            )
        }),
        ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
            let InterpolatedStringPart::Expr { expr } = part else {
                return None;
            };
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }),
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. }
        | ExprKind::MemberAccess { receiver: expr, .. } => {
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            stable_closure_lexical_path_in_expr_impl(lhs, target_span, prefix, next_closure_index)
                .or_else(|| {
                    stable_closure_lexical_path_in_expr_impl(
                        rhs,
                        target_span,
                        prefix,
                        next_closure_index,
                    )
                })
        }
        ExprKind::Block(block) => {
            stable_closure_lexical_path_in_block(block, target_span, prefix, next_closure_index)
        }
        ExprKind::Closure(closure) => {
            let path = stable_closure_child_path(prefix, *next_closure_index);
            *next_closure_index += 1;
            if closure.span == target_span {
                return Some(path);
            }
            let mut nested_closure_index = 0;
            stable_closure_lexical_path_in_expr_impl(
                &closure.body,
                target_span,
                Some(path.as_str()),
                &mut nested_closure_index,
            )
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            stable_closure_lexical_path_in_expr_impl(cond, target_span, prefix, next_closure_index)
                .or_else(|| {
                    stable_closure_lexical_path_in_expr_impl(
                        then_branch,
                        target_span,
                        prefix,
                        next_closure_index,
                    )
                })
                .or_else(|| {
                    else_branch.as_ref().and_then(|expr| {
                        stable_closure_lexical_path_in_expr_impl(
                            expr,
                            target_span,
                            prefix,
                            next_closure_index,
                        )
                    })
                })
        }
        ExprKind::When { subject, arms } => stable_closure_lexical_path_in_expr_impl(
            subject,
            target_span,
            prefix,
            next_closure_index,
        )
        .or_else(|| {
            arms.iter().find_map(|arm| {
                arm.guard
                    .as_ref()
                    .and_then(|guard| {
                        stable_closure_lexical_path_in_expr_impl(
                            guard,
                            target_span,
                            prefix,
                            next_closure_index,
                        )
                    })
                    .or_else(|| {
                        stable_closure_lexical_path_in_expr_impl(
                            &arm.body,
                            target_span,
                            prefix,
                            next_closure_index,
                        )
                    })
            })
        }),
        ExprKind::Call { callee, args } => stable_closure_lexical_path_in_expr_impl(
            callee,
            target_span,
            prefix,
            next_closure_index,
        )
        .or_else(|| {
            args.iter().find_map(|arg| {
                stable_closure_lexical_path_in_call_arg(
                    arg,
                    target_span,
                    prefix,
                    next_closure_index,
                )
            })
        }),
        ExprKind::Perform { args, .. } => args.iter().find_map(|arg| {
            stable_closure_lexical_path_in_call_arg(arg, target_span, prefix, next_closure_index)
        }),
        ExprKind::Handle(handle) => stable_closure_lexical_path_in_block(
            &handle.body,
            target_span,
            prefix,
            next_closure_index,
        )
        .or_else(|| {
            handle.arms.iter().find_map(|arm| {
                stable_closure_lexical_path_in_expr_impl(
                    &arm.body,
                    target_span,
                    prefix,
                    next_closure_index,
                )
            })
        })
        .or_else(|| {
            handle.finally.as_ref().and_then(|block| {
                stable_closure_lexical_path_in_block(block, target_span, prefix, next_closure_index)
            })
        }),
    }
}

fn stable_closure_lexical_path_in_call_arg(
    arg: &CallArg,
    target_span: Span,
    prefix: Option<&str>,
    next_closure_index: &mut usize,
) -> Option<String> {
    match arg {
        CallArg::Positional(expr) => {
            stable_closure_lexical_path_in_expr_impl(expr, target_span, prefix, next_closure_index)
        }
        CallArg::Named { value, .. } => {
            stable_closure_lexical_path_in_expr_impl(value, target_span, prefix, next_closure_index)
        }
    }
}

fn stable_closure_child_path(prefix: Option<&str>, ordinal: usize) -> String {
    match prefix {
        Some(prefix) => format!("{prefix}.$lambda{ordinal}"),
        None => format!("$lambda{ordinal}"),
    }
}
