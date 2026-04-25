//! Shared concrete-type / field-type expression fact resolution.
//!
//! 这里把“如何基于 `ProgramFacts` + `TypeStore` 从 HIR 表达式恢复 concrete type”
//! 收口成 backend-agnostic 的公共 helper，避免 LLVM generic lowering 与
//! effect/state-machine shared analysis 各自维护一套平行实现。

use crate::hir;
use crate::program_facts::ProgramFacts;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

/// 返回一个 HIR type 是否已经足够精确，可直接作为 concrete type 使用。
pub(crate) fn hir_ty_is_precise(types: &TypeStore, ty: TypeId) -> bool {
    !matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Param(_)
    )
}

/// 基于 shared `ProgramFacts` 恢复表达式 concrete type 的轻量 resolver。
///
/// `local_ty_lookup` 只负责回答“当前作用域中某个 local 的已知 concrete type 是什么”，
/// 其余 top-level value / object property / struct/class field / function return 等共享事实
/// 全部来自 `ProgramFacts`。
pub(crate) struct ExprFactResolver<'a, LocalTyLookup> {
    types: &'a TypeStore,
    program_facts: &'a ProgramFacts,
    local_ty_lookup: LocalTyLookup,
}

impl<'a, LocalTyLookup> ExprFactResolver<'a, LocalTyLookup>
where
    LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
{
    pub(crate) fn new(
        types: &'a TypeStore,
        program_facts: &'a ProgramFacts,
        local_ty_lookup: LocalTyLookup,
    ) -> Self {
        Self {
            types,
            program_facts,
            local_ty_lookup,
        }
    }

    /// 从表达式尽量恢复 exact/concrete 的 `TypeId`。
    pub(crate) fn resolve_expr_concrete_type(&self, expr: &hir::Expr) -> Option<TypeId> {
        if hir_ty_is_precise(self.types, expr.ty) {
            return Some(expr.ty);
        }

        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => (self.local_ty_lookup)(*id),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.program_facts.top_level_value_ty(fqn)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.resolve_member_access_concrete_type(receiver, member)
            }
            hir::ExprKind::Call { callee, .. } => self.resolve_call_result_type(callee),
            hir::ExprKind::Block(block) => block.stmts.last().and_then(|stmt| {
                let hir::StmtKind::Expr(expr) = &stmt.kind else {
                    return None;
                };
                self.resolve_expr_concrete_type(expr)
            }),
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => else_branch.as_deref().and_then(|else_branch| {
                self.resolve_common_branch_concrete_type([then_branch.as_ref(), else_branch])
            }),
            hir::ExprKind::When { arms, .. } => {
                self.resolve_common_branch_concrete_type(arms.iter().map(|arm| &arm.body))
            }
            _ => None,
        }
    }

    fn resolve_common_branch_concrete_type<'b>(
        &self,
        exprs: impl IntoIterator<Item = &'b hir::Expr>,
    ) -> Option<TypeId> {
        let mut candidate = None;
        for expr in exprs {
            let resolved = self.resolve_expr_concrete_type(expr)?;
            match candidate {
                None => candidate = Some(resolved),
                Some(existing) if existing == resolved => {}
                Some(_) => return None,
            }
        }
        candidate
    }

    fn resolve_member_access_concrete_type(
        &self,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Option<TypeId> {
        let field_fqn = match member.resolved.as_ref()? {
            hir::MemberRef::Value { fqn, .. } | hir::MemberRef::ExtensionValue { fqn, .. } => fqn,
            _ => return None,
        };

        if let Some(ty) = self.program_facts.top_level_value_ty(field_fqn) {
            return Some(ty);
        }
        if let Some(ty) = self.program_facts.object_property_ty(field_fqn) {
            return Some(ty);
        }

        let receiver_ty = self
            .resolve_expr_concrete_type(receiver)
            .unwrap_or(receiver.ty);
        self.program_facts
            .resolve_nominal_field_ty(self.types, receiver_ty, field_fqn)
    }

    fn resolve_call_result_type(&self, callee: &hir::Expr) -> Option<TypeId> {
        if let Some(callee_ty) = self.resolve_expr_concrete_type(callee)
            && let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee_ty)
            && hir_ty_is_precise(self.types, fun_ty.return_ty)
        {
            return Some(fun_ty.return_ty);
        }

        let fqn = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
            hir::ExprKind::UnresolvedIdent { name } => Some(name.as_str()),
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                hir::MemberRef::Fun { fqn, .. } | hir::MemberRef::ExtensionFun { fqn, .. } => {
                    Some(fqn.as_str())
                }
                _ => None,
            },
            _ => None,
        }?;

        if let Some(return_ty) = self.program_facts.fun_return_ty(fqn)
            && hir_ty_is_precise(self.types, return_ty)
        {
            return Some(return_ty);
        }

        if let Some(class_ty) = self.types.find_nominal_ref_by_fqn(fqn) {
            return Some(class_ty);
        }

        self.types.iter_ids().find(|id| {
            matches!(
                self.types.kind(*id),
                TypeKind::Value(ValueTypeKind::Nominal(nominal))
                    if nominal.fqn == fqn && nominal.args.is_empty()
            )
        })
    }
}
