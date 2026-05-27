//! Post-FnLowering helpers: parse_tuple_member_index and runtime-error/runtime-type helpers.

#![allow(dead_code)]

use super::*;

pub(in crate::mir::lower) fn parse_tuple_member_index(text: &str) -> Option<usize> {
    if text.is_empty() || !text.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
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
