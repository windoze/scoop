//! Complete declaration-domain transcripts, independent of candidate sections.
use crate::*;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod binding;
mod wire;
pub use binding::*;
pub use wire::*;

/// Every source table is mandatory. Parameter calling facts belong to the
/// defaults domain and must be supplied explicitly during artifact binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclarationSourceEntriesV1 {
    pub required_protected: CanonicalProtectedDeclarationRefsV1,
    pub nominals: CanonicalNominalSourceContractsV1,
    pub constructors: CanonicalNominalSourceConstructorsV1,
    pub properties: CanonicalNominalSourcePropertiesV1,
    pub callables: CanonicalNominalSourceCallablesV1,
    pub inheritance: CanonicalSourceInheritanceInventoriesV1,
    pub interfaces: CanonicalInterfaceSourceDispatchesV1,
    pub selections: CanonicalInheritanceSourceSlotSelectionsV1,
    pub dispatch_callables: CanonicalInheritanceSourceCallablesV1,
}

/// Canonical constituents, with no semantic or consumer lookup capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclarationSourceAuthorityV1 {
    entries: TypeDeclarationSourceEntriesV1,
}
impl TypeDeclarationSourceAuthorityV1 {
    pub fn new(entries: TypeDeclarationSourceEntriesV1) -> Self {
        Self { entries }
    }
    pub const fn entries(&self) -> &TypeDeclarationSourceEntriesV1 {
        &self.entries
    }
    pub fn into_entries(self) -> TypeDeclarationSourceEntriesV1 {
        self.entries
    }
}
