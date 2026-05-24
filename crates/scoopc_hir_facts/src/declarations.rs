//! Declaration and entity facts published by the HIR semantic barrier.

use scoopc_ids::CanonicalTextKey;
use scoopc_source::SourceMapSpan;
use scoopc_types::{EffectRow, TypeId};

use crate::common::FactIdentity;

/// HIR-owned facts about nominal declarations, callables, fields, and dispatch tables.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DeclarationFacts {
    pub nominals: Vec<NominalDeclarationFact>,
    pub callables: Vec<CallableDeclarationFact>,
    pub fields: Vec<FieldDeclarationFact>,
    pub enum_variants: Vec<EnumVariantDeclarationFact>,
    pub dispatch: DispatchFacts,
}

impl DeclarationFacts {
    /// Return whether no declaration facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.nominals.is_empty()
            && self.callables.is_empty()
            && self.fields.is_empty()
            && self.enum_variants.is_empty()
            && self.dispatch.is_empty()
    }
}

/// Coarse declaration family for nominal entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NominalKind {
    Struct,
    Enum,
    Class,
    Object,
    Interface,
    Effect,
}

/// Variance attached to a type or effect parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Variance {
    Invariant,
    Covariant,
    Contravariant,
}

/// Stable type/effect parameter metadata for declaration facts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeParameterFact {
    pub key: CanonicalTextKey,
    pub name: String,
    pub variance: Variance,
    pub source: Option<SourceMapSpan>,
}

/// Fact describing a nominal declaration and its direct type hierarchy.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NominalDeclarationFact {
    pub identity: FactIdentity,
    pub kind: NominalKind,
    pub type_params: Vec<TypeParameterFact>,
    pub direct_supertypes: Vec<CanonicalTextKey>,
}

/// Fact describing a body-bearing or declared callable signature.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CallableDeclarationFact {
    pub identity: FactIdentity,
    pub receiver_ty: Option<TypeId>,
    pub parameter_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
    pub type_params: Vec<TypeParameterFact>,
    pub has_body: bool,
}

/// Source-level family for a field or property owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FieldOwnerKind {
    Struct,
    Class,
    Object,
    EnumVariant,
}

/// Fact describing a field or property type owned by a nominal/object declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FieldDeclarationFact {
    pub identity: FactIdentity,
    pub owner: CanonicalTextKey,
    pub owner_kind: FieldOwnerKind,
    pub name: String,
    pub ty: TypeId,
    pub source: Option<SourceMapSpan>,
}

/// Fact describing one enum variant and its stable tag.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnumVariantDeclarationFact {
    pub identity: FactIdentity,
    pub enum_owner: CanonicalTextKey,
    pub name: String,
    pub tag: u64,
    pub source: Option<SourceMapSpan>,
}

/// Dispatch metadata that remains source-semantic rather than MIR-derived.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DispatchFacts {
    pub vtables: Vec<DispatchTableFact>,
    pub interface_tables: Vec<DispatchTableFact>,
}

impl DispatchFacts {
    /// Return whether no dispatch facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.vtables.is_empty() && self.interface_tables.is_empty()
    }
}

/// Stable slot table for virtual or interface dispatch.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DispatchTableFact {
    pub owner: CanonicalTextKey,
    pub slots: Vec<DispatchSlotFact>,
}

/// One source-level dispatch slot resolved during the HIR barrier.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DispatchSlotFact {
    pub index: u32,
    pub declaration: CanonicalTextKey,
    pub signature_ty: TypeId,
}
