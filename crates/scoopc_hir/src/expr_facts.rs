//! Shared declaration fact helpers used by MIR lowering and effect analysis.

use crate::ast;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};
use scoopc_hir_facts::HirFacts;
use scoopc_hir_facts::declarations::{FieldOwnerKind, NominalKind};

/// 返回一个 HIR type 是否已经足够精确，可直接作为 concrete type 使用。
pub fn hir_ty_is_precise(types: &TypeStore, ty: TypeId) -> bool {
    !matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Param(_)
    )
}

/// Query helper for declaration/entity facts published by the HIR barrier.
pub struct HirFactResolver<'a> {
    hir_facts: &'a HirFacts,
}

impl<'a> HirFactResolver<'a> {
    pub fn new(_types: &'a TypeStore, hir_facts: &'a HirFacts) -> Self {
        Self { hir_facts }
    }

    pub fn member_value_tys(&self) -> Vec<(String, TypeId)> {
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

    pub fn nominal_kinds(&self) -> Vec<(String, ast::TypeKind)> {
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

    pub fn enum_payload_kinds(&self) -> Vec<(String, bool)> {
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
}
