//! Closure capture computation, declared/used local collection.

#![allow(dead_code)]

use super::*;

pub(in crate::hir::lower) fn compute_closure_captures(
    params: &[Param],
    body: &crate::hir::Expr,
    local_mutability: &HashMap<SymbolId, bool>,
) -> Vec<Capture> {
    let mut declared: HashSet<SymbolId> = params.iter().map(|p| p.id).collect();
    collect_declared_locals_in_expr(body, &mut declared);

    let mut used: HashMap<SymbolId, Capture> = HashMap::new();
    collect_used_locals_in_expr(body, &mut used);

    let mut captures: Vec<Capture> = used
        .into_values()
        .filter(|c| !declared.contains(&c.id))
        .collect();

    for c in &mut captures {
        c.mutable = local_mutability.get(&c.id).copied().unwrap_or(false);
    }

    // 稳定排序：按声明位置排序（同位置用 SymbolId 兜底）。
    captures.sort_by(|a, b| {
        a.decl_span
            .start
            .cmp(&b.decl_span.start)
            .then_with(|| a.decl_span.end.cmp(&b.decl_span.end))
            .then_with(|| a.id.as_u32().cmp(&b.id.as_u32()))
    });

    captures
}

pub(in crate::hir::lower) fn collect_declared_locals_in_expr(
    expr: &crate::hir::Expr,
    declared: &mut HashSet<SymbolId>,
) {
    match &expr.kind {
        crate::hir::ExprKind::Missing
        | crate::hir::ExprKind::Literal(_)
        | crate::hir::ExprKind::VarRef(_)
        | crate::hir::ExprKind::UnresolvedIdent { .. }
        | crate::hir::ExprKind::ClassLiteral(_)
        | crate::hir::ExprKind::Todo(_) => {}
        crate::hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_declared_locals_in_expr(&f.value, declared);
            }
        }
        crate::hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        crate::hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let InterpolatedStringPart::Expr { expr } = p {
                    collect_declared_locals_in_expr(expr, declared);
                }
            }
        }
        crate::hir::ExprKind::Unary { expr, .. } => {
            collect_declared_locals_in_expr(expr.as_ref(), declared)
        }
        crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_declared_locals_in_expr(lhs.as_ref(), declared);
            collect_declared_locals_in_expr(rhs.as_ref(), declared);
        }
        crate::hir::ExprKind::TypeCheck { expr, .. } | crate::hir::ExprKind::Cast { expr, .. } => {
            collect_declared_locals_in_expr(expr.as_ref(), declared);
        }
        crate::hir::ExprKind::Block(block) => collect_declared_locals_in_block(block, declared),
        crate::hir::ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        crate::hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_declared_locals_in_expr(cond, declared);
            collect_declared_locals_in_expr(then_branch, declared);
            if let Some(e) = else_branch.as_deref() {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        crate::hir::ExprKind::When { subject, arms } => {
            collect_declared_locals_in_expr(subject, declared);
            for arm in arms {
                collect_declared_locals_in_when_pat(&arm.pat, declared);
                if let Some(g) = &arm.guard {
                    collect_declared_locals_in_expr(g, declared);
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
        }
        crate::hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_declared_locals_in_expr(receiver, declared)
        }
        crate::hir::ExprKind::Call { callee, args } => {
            collect_declared_locals_in_expr(callee, declared);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        crate::hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        crate::hir::ExprKind::Handle(handle) => {
            collect_declared_locals_in_block(&handle.body, declared);
            for arm in &handle.arms {
                for b in &arm.op.binders {
                    declared.insert(b.id);
                }
                match arm.kind {
                    crate::hir::HandleArmKind::EscapeContinuation { continuation } => {
                        declared.insert(continuation);
                    }
                    crate::hir::HandleArmKind::NonResuming => {}
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
            if let Some(finally) = &handle.finally {
                collect_declared_locals_in_block(finally, declared);
            }
        }
    }
}

pub(in crate::hir::lower) fn collect_declared_locals_in_block(
    block: &Block,
    declared: &mut HashSet<SymbolId>,
) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Val(v) => {
                if let Some(id) = v.id {
                    declared.insert(id);
                }
                if let Some(init) = &v.init {
                    collect_declared_locals_in_expr(init, declared);
                }
            }
            StmtKind::Expr(e) => collect_declared_locals_in_expr(e, declared),
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_declared_locals_in_expr(lhs, declared);
                collect_declared_locals_in_expr(rhs, declared);
            }
            StmtKind::While { cond, body } => {
                collect_declared_locals_in_expr(cond, declared);
                collect_declared_locals_in_block(body, declared);
            }
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_declared_locals_in_expr(v, declared);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_declared_locals_in_when_pat(
    pat: &WhenPat,
    declared: &mut HashSet<SymbolId>,
) {
    match pat {
        WhenPat::Or { pats, .. } => {
            for p in pats {
                collect_declared_locals_in_when_pat(p, declared);
            }
        }
        WhenPat::Bind { id, .. } => {
            declared.insert(*id);
        }
        WhenPat::Tuple { elements, .. } => {
            for e in elements {
                collect_declared_locals_in_when_pat(e, declared);
            }
        }
        WhenPat::Variant { args, .. } => {
            for a in args {
                collect_declared_locals_in_when_pat(a, declared);
            }
        }
        WhenPat::Else { .. }
        | WhenPat::Wildcard { .. }
        | WhenPat::Rest { .. }
        | WhenPat::Is { .. }
        | WhenPat::IntLit { .. }
        | WhenPat::CharLit { .. }
        | WhenPat::StringLit { .. }
        | WhenPat::BoolLit { .. } => {}
    }
}

pub(in crate::hir::lower) fn collect_used_locals_in_expr(
    expr: &crate::hir::Expr,
    used: &mut HashMap<SymbolId, Capture>,
) {
    match &expr.kind {
        crate::hir::ExprKind::Missing
        | crate::hir::ExprKind::Literal(_)
        | crate::hir::ExprKind::UnresolvedIdent { .. }
        | crate::hir::ExprKind::ClassLiteral(_)
        | crate::hir::ExprKind::Todo(_) => {}
        crate::hir::ExprKind::VarRef(v) => {
            let ValueRef::Local {
                id,
                name,
                decl_span,
            } = v
            else {
                return;
            };
            used.entry(*id).or_insert_with(|| Capture {
                id: *id,
                name: name.clone(),
                decl_span: *decl_span,
                mutable: false,
            });
        }
        crate::hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_used_locals_in_expr(&f.value, used);
            }
        }
        crate::hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_used_locals_in_expr(e, used);
            }
        }
        crate::hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let InterpolatedStringPart::Expr { expr } = p {
                    collect_used_locals_in_expr(expr, used);
                }
            }
        }
        crate::hir::ExprKind::Unary { expr, .. } => {
            collect_used_locals_in_expr(expr.as_ref(), used)
        }
        crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_locals_in_expr(lhs.as_ref(), used);
            collect_used_locals_in_expr(rhs.as_ref(), used);
        }
        crate::hir::ExprKind::TypeCheck { expr, .. } | crate::hir::ExprKind::Cast { expr, .. } => {
            collect_used_locals_in_expr(expr.as_ref(), used);
        }
        crate::hir::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StmtKind::Expr(e) => collect_used_locals_in_expr(e, used),
                    StmtKind::Val(v) => {
                        if let Some(init) = &v.init {
                            collect_used_locals_in_expr(init, used);
                        }
                    }
                    StmtKind::Assign { lhs, rhs, .. } => {
                        collect_used_locals_in_expr(lhs, used);
                        collect_used_locals_in_expr(rhs, used);
                    }
                    StmtKind::While { cond, body } => {
                        collect_used_locals_in_expr(cond, used);
                        // while body 是一个 block；其内部的局部声明不影响“使用”收集。
                        collect_used_locals_in_expr(
                            &crate::hir::Expr {
                                span: body.span,
                                ty: body.ty,
                                kind: crate::hir::ExprKind::Block(body.clone()),
                            },
                            used,
                        );
                    }
                    StmtKind::Return { value } => {
                        if let Some(v) = value {
                            collect_used_locals_in_expr(v, used);
                        }
                    }
                    StmtKind::Empty
                    | StmtKind::Break { .. }
                    | StmtKind::Continue { .. }
                    | StmtKind::Todo(_) => {}
                }
            }
        }
        crate::hir::ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        crate::hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_locals_in_expr(cond, used);
            collect_used_locals_in_expr(then_branch, used);
            if let Some(e) = else_branch.as_deref() {
                collect_used_locals_in_expr(e, used);
            }
        }
        crate::hir::ExprKind::When { subject, arms } => {
            collect_used_locals_in_expr(subject, used);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_used_locals_in_expr(g, used);
                }
                collect_used_locals_in_expr(&arm.body, used);
            }
        }
        crate::hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_used_locals_in_expr(receiver, used)
        }
        crate::hir::ExprKind::Call { callee, args } => {
            collect_used_locals_in_expr(callee, used);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        crate::hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        crate::hir::ExprKind::Handle(handle) => {
            // handle body / arm body 里的 var refs 都算“使用”；binder 是否 capture 由 declared 集合处理。
            collect_used_locals_in_expr(
                &crate::hir::Expr {
                    span: handle.body.span,
                    ty: handle.body.ty,
                    kind: crate::hir::ExprKind::Block(handle.body.clone()),
                },
                used,
            );
            for arm in &handle.arms {
                collect_used_locals_in_expr(&arm.body, used);
            }
            if let Some(finally) = &handle.finally {
                collect_used_locals_in_expr(
                    &crate::hir::Expr {
                        span: finally.span,
                        ty: finally.ty,
                        kind: crate::hir::ExprKind::Block(finally.clone()),
                    },
                    used,
                );
            }
        }
    }
}
