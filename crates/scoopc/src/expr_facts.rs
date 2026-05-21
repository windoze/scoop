//! Shared concrete-type / field-type expression fact resolution.
//!
//! 这里把“如何基于 `HirFacts` + `TypeStore` 从 HIR 表达式恢复 concrete type”
//! 收口成 backend-agnostic 的公共 helper，避免 LLVM generic lowering 与
//! effect/state-machine shared analysis 各自维护一套平行实现。

use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use crate::{ast, hir};
use scoopc_hir_facts::HirFacts;
use scoopc_hir_facts::declarations::{FieldOwnerKind, NominalKind};
use scoopc_hir_facts::globals::GlobalRootKind;

/// 返回一个 HIR type 是否已经足够精确，可直接作为 concrete type 使用。
pub(crate) fn hir_ty_is_precise(types: &TypeStore, ty: TypeId) -> bool {
    !matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Param(_)
    )
}

/// 基于 HIR declaration facts 恢复表达式 concrete type 的轻量 resolver。
///
/// `local_ty_lookup` 只负责回答“当前作用域中某个 local 的已知 concrete type 是什么”，
/// 其余 top-level value / object property / struct/class field / function return 等共享事实
/// 全部来自 `HirFacts`。
pub(crate) struct ExprFactResolver<'a, LocalTyLookup> {
    types: &'a TypeStore,
    hir_facts: &'a HirFacts,
    local_ty_lookup: LocalTyLookup,
}

/// Query helper for declaration/entity facts published by the HIR barrier.
pub(crate) struct HirFactResolver<'a> {
    types: &'a TypeStore,
    hir_facts: &'a HirFacts,
}

impl<'a> HirFactResolver<'a> {
    pub(crate) fn new(types: &'a TypeStore, hir_facts: &'a HirFacts) -> Self {
        Self { types, hir_facts }
    }

    pub(crate) fn top_level_value_ty(&self, fqn: &str) -> Option<TypeId> {
        self.hir_facts
            .globals
            .roots
            .iter()
            .find(|root| {
                matches!(
                    root.kind,
                    GlobalRootKind::TopLevelVal | GlobalRootKind::TopLevelVar
                ) && root.identity.display_name == fqn
            })
            .and_then(|root| root.ty)
    }

    #[cfg(feature = "llvm")]
    pub(crate) fn extern_global_ty(&self, fqn: &str) -> Option<TypeId> {
        self.hir_facts
            .native
            .extern_globals
            .iter()
            .find(|global| global.identity.display_name == fqn)
            .map(|global| global.ty)
    }

    #[cfg(feature = "llvm")]
    pub(crate) fn has_extern_global(&self, fqn: &str) -> bool {
        self.hir_facts
            .native
            .extern_globals
            .iter()
            .any(|global| global.identity.display_name == fqn)
    }

    pub(crate) fn object_property_ty(&self, fqn: &str) -> Option<TypeId> {
        self.hir_facts
            .declarations
            .fields
            .iter()
            .find(|field| {
                field.owner_kind == FieldOwnerKind::Object && field.identity.display_name == fqn
            })
            .map(|field| field.ty)
    }

    pub(crate) fn fun_return_ty(&self, fqn: &str) -> Option<TypeId> {
        self.hir_facts
            .declarations
            .callables
            .iter()
            .find(|callable| callable.identity.display_name == fqn)
            .map(|callable| callable.return_ty)
    }

    pub(crate) fn resolve_nominal_field_ty(
        &self,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        self.resolve_struct_field_ty(receiver_ty, field_fqn)
            .or_else(|| self.resolve_class_field_ty(receiver_ty, field_fqn))
    }

    pub(crate) fn is_object_value_fqn(&self, fqn: &str) -> bool {
        self.hir_facts.globals.roots.iter().any(|root| {
            root.kind == GlobalRootKind::ObjectSingleton && root.identity.display_name == fqn
        })
    }

    pub(crate) fn is_object_property_fqn(&self, fqn: &str) -> bool {
        self.hir_facts.declarations.fields.iter().any(|field| {
            field.owner_kind == FieldOwnerKind::Object && field.identity.display_name == fqn
        })
    }

    pub(crate) fn is_top_level_immutable_value_fqn(&self, fqn: &str) -> bool {
        self.hir_facts.globals.roots.iter().any(|root| {
            root.kind == GlobalRootKind::TopLevelVal && root.identity.display_name == fqn
        })
    }

    pub(crate) fn member_value_tys(&self) -> Vec<(String, TypeId)> {
        self.hir_facts
            .declarations
            .fields
            .iter()
            .filter(|field| {
                matches!(
                    field.owner_kind,
                    FieldOwnerKind::Struct | FieldOwnerKind::Class | FieldOwnerKind::Object
                )
            })
            .map(|field| (field.identity.display_name.clone(), field.ty))
            .collect()
    }

    pub(crate) fn nominal_kinds(&self) -> Vec<(String, ast::TypeKind)> {
        self.hir_facts
            .declarations
            .nominals
            .iter()
            .filter_map(|nominal| {
                let kind = match nominal.kind {
                    NominalKind::Struct => ast::TypeKind::Struct,
                    NominalKind::Enum => ast::TypeKind::Enum,
                    NominalKind::Class => ast::TypeKind::Class,
                    NominalKind::Interface => ast::TypeKind::Interface,
                    NominalKind::Effect => ast::TypeKind::Effect,
                    NominalKind::Object => return None,
                };
                Some((nominal.identity.display_name.clone(), kind))
            })
            .collect()
    }

    pub(crate) fn enum_payload_kinds(&self) -> Vec<(String, bool)> {
        self.hir_facts
            .declarations
            .nominals
            .iter()
            .filter(|nominal| nominal.kind == NominalKind::Enum)
            .map(|nominal| {
                let enum_fqn = nominal.identity.display_name.as_str();
                let has_payload = self
                    .hir_facts
                    .declarations
                    .enum_variants
                    .iter()
                    .filter(|variant| variant.enum_owner.as_str() == enum_fqn)
                    .any(|variant| {
                        self.hir_facts.declarations.fields.iter().any(|field| {
                            field.owner_kind == FieldOwnerKind::EnumVariant
                                && field.owner.as_str() == variant.identity.display_name
                        })
                    });
                (nominal.identity.display_name.clone(), has_payload)
            })
            .collect()
    }

    fn resolve_struct_field_ty(&self, receiver_ty: TypeId, field_fqn: &str) -> Option<TypeId> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, self.types);
        self.lookup_field_ty_by_owner(FieldOwnerKind::Struct, &layout_key, field_fqn)
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| {
                        self.lookup_field_ty_by_owner(
                            FieldOwnerKind::Struct,
                            &nominal.fqn,
                            field_fqn,
                        )
                    })
                    .flatten()
            })
    }

    fn resolve_class_field_ty(&self, receiver_ty: TypeId, field_fqn: &str) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, self.types);
        self.lookup_class_field_ty_by_key(&layout_key, field_fqn)
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| self.lookup_class_field_ty_by_key(&nominal.fqn, field_fqn))
                    .flatten()
            })
    }

    fn lookup_class_field_ty_by_key(&self, class_key: &str, field_fqn: &str) -> Option<TypeId> {
        self.lookup_field_ty_by_owner(FieldOwnerKind::Class, class_key, field_fqn)
            .or_else(|| {
                self.hir_facts
                    .declarations
                    .nominals
                    .iter()
                    .find(|nominal| nominal.identity.display_name == class_key)
                    .and_then(|nominal| nominal.direct_supertypes.first())
                    .and_then(|super_key| {
                        self.lookup_class_field_ty_by_key(super_key.as_str(), field_fqn)
                    })
            })
    }

    fn lookup_field_ty_by_owner(
        &self,
        owner_kind: FieldOwnerKind,
        owner_key: &str,
        field_fqn: &str,
    ) -> Option<TypeId> {
        self.hir_facts
            .declarations
            .fields
            .iter()
            .find(|field| {
                field.owner_kind == owner_kind
                    && field.owner.as_str() == owner_key
                    && field.identity.display_name == field_fqn
            })
            .map(|field| field.ty)
    }
}

impl<'a, LocalTyLookup> ExprFactResolver<'a, LocalTyLookup>
where
    LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
{
    pub(crate) fn new(
        types: &'a TypeStore,
        hir_facts: &'a HirFacts,
        local_ty_lookup: LocalTyLookup,
    ) -> Self {
        Self {
            types,
            hir_facts,
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
                self.query().top_level_value_ty(fqn)
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

        if let Some(ty) = self.query().top_level_value_ty(field_fqn) {
            return Some(ty);
        }
        if let Some(ty) = self.query().object_property_ty(field_fqn) {
            return Some(ty);
        }

        let receiver_ty = self
            .resolve_expr_concrete_type(receiver)
            .unwrap_or(receiver.ty);
        self.query()
            .resolve_nominal_field_ty(receiver_ty, field_fqn)
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

        if let Some(return_ty) = self.query().fun_return_ty(fqn)
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

    fn query(&self) -> HirFactResolver<'_> {
        HirFactResolver::new(self.types, self.hir_facts)
    }
}
