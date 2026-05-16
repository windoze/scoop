//!  Local-id and signature collection helpers: declared/used locals, structural-signature payload helpers.

#![allow(dead_code)]

use super::*;

pub(crate) fn collect_declared_local_ids_in_stmt(
    stmt: &hir::Stmt,
    out: &mut HashSet<hir::SymbolId>,
) {
    match &stmt.kind {
        hir::StmtKind::Val(decl) => {
            if let Some(id) = decl.id {
                out.insert(id);
            }
            if let Some(init) = decl.init.as_ref() {
                collect_declared_local_ids_in_expr(init, out);
            }
        }
        hir::StmtKind::Expr(expr) => collect_declared_local_ids_in_expr(expr, out),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_declared_local_ids_in_expr(lhs, out);
            collect_declared_local_ids_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_declared_local_ids_in_expr(cond, out);
            for stmt in &body.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_declared_local_ids_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

pub(crate) fn collect_declared_local_ids_in_expr(
    expr: &hir::Expr,
    out: &mut HashSet<hir::SymbolId>,
) {
    match &expr.kind {
        hir::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_declared_local_ids_in_expr(cond, out);
            collect_declared_local_ids_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_declared_local_ids_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_declared_local_ids_in_expr(subject, out);
            for arm in arms {
                collect_declared_local_ids_in_when_pat(&arm.pat, out);
                if let Some(guard) = arm.guard.as_ref() {
                    collect_declared_local_ids_in_expr(guard, out);
                }
                collect_declared_local_ids_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_declared_local_ids_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_declared_local_ids_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_declared_local_ids_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_declared_local_ids_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_declared_local_ids_in_expr(lhs, out);
            collect_declared_local_ids_in_expr(rhs, out);
        }
        hir::ExprKind::Call { callee, args } => {
            collect_declared_local_ids_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_declared_local_ids_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_declared_local_ids_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Closure(closure) => {
            collect_declared_local_ids_in_closure(closure, out);
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &handle.body.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
            for arm in &handle.arms {
                for binder in &arm.op.binders {
                    out.insert(binder.id);
                }
                match arm.kind {
                    hir::HandleArmKind::NonResuming => {}
                    hir::HandleArmKind::EscapeContinuation { continuation } => {
                        out.insert(continuation);
                    }
                }
                collect_declared_local_ids_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                for stmt in &finally.stmts {
                    collect_declared_local_ids_in_stmt(stmt, out);
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_declared_local_ids_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_declared_local_ids_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
    }
}

pub(crate) fn collect_declared_local_ids_in_closure(
    closure: &hir::ClosureExpr,
    out: &mut HashSet<hir::SymbolId>,
) {
    for param in &closure.params {
        out.insert(param.id);
    }

    // Resolver always introduces the implicit single-argument lambda binder
    // `it` at a synthetic zero-width span anchored to the lambda start.
    // When outer-scope slot collection walks into the closure body, treat that
    // synthetic binder like any other declared local so enclosing handle/try
    // frames do not attempt to seed it from the outer env.
    if closure.params.is_empty() {
        let implicit_it_decl_span = crate::span::Span::new(closure.span.start, closure.span.start);
        for capture in &closure.captures {
            if capture.name == "it" && capture.decl_span == implicit_it_decl_span {
                out.insert(capture.id);
            }
        }
    }

    collect_declared_local_ids_in_expr(&closure.body, out);
}

pub(crate) fn collect_declared_local_ids_in_when_pat(
    pat: &hir::WhenPat,
    out: &mut HashSet<hir::SymbolId>,
) {
    match pat {
        hir::WhenPat::Or { pats, .. } => {
            for pat in pats {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Bind { id, .. } => {
            out.insert(*id);
        }
        hir::WhenPat::Tuple { elements, .. } => {
            for pat in elements {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Variant { args, .. } => {
            for pat in args {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Else { .. }
        | hir::WhenPat::Wildcard { .. }
        | hir::WhenPat::Rest { .. }
        | hir::WhenPat::Is { .. }
        | hir::WhenPat::IntLit { .. }
        | hir::WhenPat::CharLit { .. }
        | hir::WhenPat::StringLit { .. }
        | hir::WhenPat::BoolLit { .. } => {}
    }
}

pub(crate) fn collect_local_refs_in_stmt(
    stmt: &hir::Stmt,
    out: &mut HashMap<hir::SymbolId, (String, TypeId)>,
) {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => collect_local_refs_in_expr(expr, out),
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_ref() {
                collect_local_refs_in_expr(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_local_refs_in_expr(lhs, out);
            collect_local_refs_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_local_refs_in_expr(cond, out);
            for stmt in &body.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_local_refs_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

pub(crate) fn collect_local_refs_in_expr(
    expr: &hir::Expr,
    out: &mut HashMap<hir::SymbolId, (String, TypeId)>,
) {
    match &expr.kind {
        hir::ExprKind::VarRef(hir::ValueRef::Local { id, name, .. }) => {
            out.entry(*id).or_insert_with(|| (name.clone(), expr.ty));
        }
        hir::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_local_refs_in_expr(cond, out);
            collect_local_refs_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_local_refs_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_local_refs_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_local_refs_in_expr(guard, out);
                }
                collect_local_refs_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::Call { callee, args } => {
            collect_local_refs_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_local_refs_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_local_refs_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_local_refs_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_local_refs_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_local_refs_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_local_refs_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_local_refs_in_expr(lhs, out);
            collect_local_refs_in_expr(rhs, out);
        }
        hir::ExprKind::Closure(closure) => {
            collect_local_refs_in_expr(&closure.body, out);
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_local_refs_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_local_refs_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &handle.body.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
            for arm in &handle.arms {
                collect_local_refs_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                for stmt in &finally.stmts {
                    collect_local_refs_in_stmt(stmt, out);
                }
            }
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
    }
}

pub(crate) fn collect_used_locals_in_block_static(
    block: &hir::Block,
    out: &mut HashSet<hir::SymbolId>,
) {
    for stmt in &block.stmts {
        collect_used_locals_in_stmt_static(stmt, out);
    }
}

pub(crate) fn collect_used_locals_in_call_args_static(
    args: &[hir::CallArg],
    out: &mut HashSet<hir::SymbolId>,
) {
    for arg in args {
        match arg {
            hir::CallArg::Positional(expr) => collect_used_locals_in_expr_static(expr, out),
            hir::CallArg::Named { value, .. } => collect_used_locals_in_expr_static(value, out),
        }
    }
}

pub(crate) fn collect_used_locals_in_handle_static(
    handle: &hir::HandleExpr,
    out: &mut HashSet<hir::SymbolId>,
) {
    collect_used_locals_in_block_static(&handle.body, out);
    for arm in &handle.arms {
        collect_used_locals_in_expr_static(&arm.body, out);
    }
    if let Some(finally) = &handle.finally {
        collect_used_locals_in_block_static(finally, out);
    }
}

pub(crate) fn collect_used_locals_in_stmt_static(
    stmt: &hir::Stmt,
    out: &mut HashSet<hir::SymbolId>,
) {
    match &stmt.kind {
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
        hir::StmtKind::Expr(expr) => collect_used_locals_in_expr_static(expr, out),
        hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_used_locals_in_expr_static(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_used_locals_in_expr_static(lhs, out);
            collect_used_locals_in_expr_static(rhs, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_used_locals_in_expr_static(expr, out);
            }
        }
        hir::StmtKind::While { cond, body } => {
            collect_used_locals_in_expr_static(cond, out);
            collect_used_locals_in_block_static(body, out);
        }
    }
}

pub(crate) fn collect_used_locals_in_expr_static(
    expr: &hir::Expr,
    out: &mut HashSet<hir::SymbolId>,
) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
            out.insert(*id);
        }
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {}
        hir::ExprKind::Call { callee, args } => {
            collect_used_locals_in_expr_static(callee, out);
            collect_used_locals_in_call_args_static(args, out);
        }
        hir::ExprKind::Perform { args, .. } => {
            collect_used_locals_in_call_args_static(args, out);
        }
        hir::ExprKind::Block(block) => {
            collect_used_locals_in_block_static(block, out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_locals_in_expr_static(cond, out);
            collect_used_locals_in_expr_static(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_used_locals_in_expr_static(else_branch, out);
            }
        }
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_locals_in_expr_static(lhs, out);
            collect_used_locals_in_expr_static(rhs, out);
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => {
            collect_used_locals_in_expr_static(inner, out);
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_used_locals_in_expr_static(expr, out);
                }
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_used_locals_in_expr_static(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_used_locals_in_expr_static(element, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_used_locals_in_expr_static(subject, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_used_locals_in_expr_static(guard, out);
                }
                collect_used_locals_in_expr_static(&arm.body, out);
            }
        }
        hir::ExprKind::Closure(closure) => {
            for capture in &closure.captures {
                out.insert(capture.id);
            }
            collect_used_locals_in_expr_static(&closure.body, out);
        }
        hir::ExprKind::Handle(handle) => {
            collect_used_locals_in_handle_static(handle, out);
        }
    }
}

#[cfg(test)]
pub(crate) fn render_symbol_list(
    ids: &[hir::SymbolId],
    slots: &HashMap<hir::SymbolId, FrameSlot>,
) -> String {
    let mut labels = ids
        .iter()
        .map(|id| {
            slots.get(id).map_or_else(
                || format!("unknown#{}", id.as_u32()),
                FrameSlot::display_name,
            )
        })
        .collect::<Vec<_>>();
    labels.sort();
    labels.join(", ")
}

#[cfg(test)]
pub(crate) fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(crate) fn expr_payload_signature(expr: &hir::Expr) -> usize {
    expr.span.start
        ^ (expr.span.end << 1)
        ^ ((expr.ty.as_u32() as usize) << 2)
        ^ expr_kind_signature(&expr.kind)
}

pub(crate) fn expr_kind_signature(kind: &hir::ExprKind) -> usize {
    match kind {
        hir::ExprKind::Missing => 1,
        hir::ExprKind::Literal(_) => 2,
        hir::ExprKind::VarRef(_) => 3,
        hir::ExprKind::UnresolvedIdent { .. } => 4,
        hir::ExprKind::StructLit { .. } => 5,
        hir::ExprKind::TupleLit { .. } => 6,
        hir::ExprKind::InterpolatedString { .. } => 7,
        hir::ExprKind::Unary { .. } => 8,
        hir::ExprKind::Binary { .. } => 9,
        hir::ExprKind::TypeCheck { .. } => 10,
        hir::ExprKind::Cast { .. } => 11,
        hir::ExprKind::Block(_) => 12,
        hir::ExprKind::Closure(_) => 13,
        hir::ExprKind::If { .. } => 14,
        hir::ExprKind::When { .. } => 15,
        hir::ExprKind::MemberAccess { .. } => 16,
        hir::ExprKind::Call { .. } => 17,
        hir::ExprKind::Perform { .. } => 18,
        hir::ExprKind::Handle(_) => 19,
        hir::ExprKind::ClassLiteral(_) => 20,
        hir::ExprKind::Todo(_) => 21,
    }
}

pub(crate) fn stmt_payload_signature(stmt: &hir::Stmt) -> usize {
    stmt.span.start
        ^ (stmt.span.end << 1)
        ^ ((stmt.ty.as_u32() as usize) << 2)
        ^ stmt_kind_signature(&stmt.kind)
}

pub(crate) fn stmt_kind_signature(kind: &hir::StmtKind) -> usize {
    match kind {
        hir::StmtKind::Empty => 1,
        hir::StmtKind::Expr(_) => 2,
        hir::StmtKind::Val(_) => 3,
        hir::StmtKind::Assign { .. } => 4,
        hir::StmtKind::While { .. } => 5,
        hir::StmtKind::Break { .. } => 6,
        hir::StmtKind::Continue { .. } => 7,
        hir::StmtKind::Return { .. } => 8,
        hir::StmtKind::Todo(_) => 9,
    }
}

pub(crate) fn decl_payload_signature(decl: &hir::ValDecl) -> usize {
    decl.span.start
        ^ (decl.span.end << 1)
        ^ decl.id.map(|id| id.as_u32() as usize).unwrap_or(0)
        ^ ((decl.ty.as_u32() as usize) << 2)
        ^ ((usize::from(decl.mutable)) << 3)
}

pub(crate) fn handle_arm_payload_signature(arm: &hir::HandleArm) -> usize {
    arm.span.start
        ^ (arm.span.end << 1)
        ^ arm.op.op.fqn.len()
        ^ handle_arm_kind_signature(arm.kind)
        ^ expr_payload_signature(&arm.body)
}

pub(crate) fn handle_arm_kind_signature(kind: hir::HandleArmKind) -> usize {
    match kind {
        hir::HandleArmKind::NonResuming => 1,
        hir::HandleArmKind::EscapeContinuation { continuation } => {
            2 ^ (continuation.as_u32() as usize)
        }
    }
}
