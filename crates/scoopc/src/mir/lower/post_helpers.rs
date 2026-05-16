//! Post-FnLowering helpers: parse_tuple_member_index, runtime-error/runtime-type helpers, boxed_symbols collection.

#![allow(dead_code)]

use super::*;

pub(in crate::mir::lower) fn parse_tuple_member_index(text: &str) -> Option<usize> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

pub(in crate::mir::lower) fn payload_tuple_ty_from_components(
    types: &mut TypeStore,
    unit_ty: TypeId,
    components: &[TypeId],
) -> Option<TypeId> {
    match components {
        [] => Some(unit_ty),
        [single] => Some(*single),
        _ => Some(types.ty_tuple(components.to_vec())),
    }
}

pub(in crate::mir::lower) fn continuation_identity_return_param(
    types: &TypeStore,
    fun: &hir::FunDecl,
) -> Option<usize> {
    continuation_contract_from_type(types, fun.return_ty)?;
    let returned = block_identity_return_expr(fun.body.as_ref()?)?;
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &returned.kind else {
        return None;
    };
    let param_index = fun.params.iter().position(|param| param.id == *id)?;
    continuation_contract_from_type(types, fun.params[param_index].ty)?;
    Some(param_index)
}

pub(in crate::mir::lower) fn block_identity_return_expr(block: &hir::Block) -> Option<&hir::Expr> {
    let [stmt] = block.stmts.as_slice() else {
        return None;
    };
    match &stmt.kind {
        hir::StmtKind::Return { value: Some(value) } | hir::StmtKind::Expr(value) => Some(value),
        _ => None,
    }
}

pub(in crate::mir::lower) fn continuation_contract_from_type(
    types: &TypeStore,
    continuation_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(continuation_ty) else {
        return None;
    };
    if nominal.fqn != "scoop.core.Continuation" || nominal.args.len() < 2 {
        return None;
    }
    Some((
        nominal.args[0],
        nominal.args[1],
        nominal.eff.clone().unwrap_or_else(EffectRow::pure),
    ))
}

pub(in crate::mir::lower) fn find_raise_runtime_error_effect(types: &TypeStore) -> Option<TypeId> {
    let runtime_error_ty = find_runtime_error_type(types)?;
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
        )
    })
}

pub(in crate::mir::lower) fn find_runtime_error_type(types: &TypeStore) -> Option<TypeId> {
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.RuntimeError"
        ) || matches!(
            types.kind(id),
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    })
}

pub(in crate::mir::lower) fn boxed_symbols_in_block(block: &hir::Block) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_block(block, &mut out);
    out
}

pub(in crate::mir::lower) fn boxed_symbols_in_expr(expr: &hir::Expr) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_expr(expr, &mut out);
    out
}

pub(in crate::mir::lower) fn collect_boxed_symbols_in_block(
    block: &hir::Block,
    out: &mut HashSet<hir::SymbolId>,
) {
    for stmt in &block.stmts {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => collect_boxed_symbols_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = &decl.init {
                    collect_boxed_symbols_in_expr(init, out);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_boxed_symbols_in_expr(lhs, out);
                collect_boxed_symbols_in_expr(rhs, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_boxed_symbols_in_expr(cond, out);
                collect_boxed_symbols_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_boxed_symbols_in_expr(v, out);
                }
            }
        }
    }
}

pub(in crate::mir::lower) fn collect_boxed_symbols_in_expr(
    expr: &hir::Expr,
    out: &mut HashSet<hir::SymbolId>,
) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_boxed_symbols_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_boxed_symbols_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_boxed_symbols_in_expr(expr.as_ref(), out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_boxed_symbols_in_expr(lhs.as_ref(), out);
            collect_boxed_symbols_in_expr(rhs.as_ref(), out);
        }
        hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
            collect_boxed_symbols_in_expr(expr.as_ref(), out);
        }
        hir::ExprKind::Block(block) => collect_boxed_symbols_in_block(block, out),
        hir::ExprKind::Closure(closure) => {
            for cap in &closure.captures {
                if cap.mutable {
                    out.insert(cap.id);
                }
            }
            collect_boxed_symbols_in_expr(closure.body.as_ref(), out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_boxed_symbols_in_expr(cond, out);
            collect_boxed_symbols_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_deref() {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_boxed_symbols_in_expr(subject, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_boxed_symbols_in_expr(g, out);
                }
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_boxed_symbols_in_expr(receiver, out)
        }
        hir::ExprKind::Call { callee, args } => {
            collect_boxed_symbols_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_boxed_symbols_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
            if let Some(finally) = &handle.finally {
                collect_boxed_symbols_in_block(finally, out);
            }
        }
    }
}
