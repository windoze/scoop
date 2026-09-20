use std::fmt;

use scoop_identity::{
    ConeIdentity, ExactTypeKey, PersistentExactTypeId, PersistentPropertyAccessorId,
    PersistentTypeId,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::*;

mod wire;
pub use wire::*;
mod binding;
#[cfg(test)]
mod tests;
mod validation;
pub use binding::*;

/// Source-side inputs with no semantic proof or consumer lookup capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFoundationSourceEntriesV1 {
    pub provider: ConeIdentity,
    pub exact_keys: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    pub sources: CanonicalTypeSourceNominalsV1,
    pub representations: CanonicalNominalRepresentationSupportV1,
    pub generated_nominals: CanonicalPersistentIdsV1<PersistentTypeId>,
    pub accessor_keys: CanonicalPersistentIdsV1<PersistentPropertyAccessorId>,
    pub definition_sources: CanonicalExportDefinitionSourcesV1,
    pub source_roots: CanonicalSourceNominalIdsV1,
    pub local_exact_facts: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    pub dependency_facts: CanonicalTypeSectionDependencyFactsV1,
    pub local_inheritance_edges: CanonicalNominalInheritanceEdgesV1,
    pub fact_shapes: CanonicalExactTypeFactShapesV1,
    pub representation_owners: CanonicalPersistentIdsV1<PersistentTypeId>,
}

/// A structurally complete transcript, distinct from checked semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFoundationSourceAuthorityV1 {
    entries: TypeFoundationSourceEntriesV1,
}

impl TypeFoundationSourceAuthorityV1 {
    pub fn try_new(
        entries: TypeFoundationSourceEntriesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, TypeFoundationSourceError> {
        validation::validate(&entries, meter)?;
        Ok(Self { entries })
    }

    pub const fn entries(&self) -> &TypeFoundationSourceEntriesV1 {
        &self.entries
    }

    pub fn into_entries(self) -> TypeFoundationSourceEntriesV1 {
        self.entries
    }
}

#[derive(Debug)]
pub enum TypeFoundationSourceError {
    Resource(WireError),
    Constituent { field: u64, reason: String },
    Inventory { field: u64 },
    FactOwnershipOverlap(PersistentExactTypeId),
    MissingSourceOwner(PersistentTypeId),
    RepresentationAccess(PersistentTypeId),
    Identity(String),
}

impl From<WireError> for TypeFoundationSourceError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for TypeFoundationSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Constituent { field, reason } => {
                write!(f, "invalid foundation source field {field}: {reason}")
            }
            Self::Inventory { field } => {
                write!(f, "foundation source inventory mismatch at field {field}")
            }
            Self::FactOwnershipOverlap(exact) => write!(
                f,
                "foundation source fact {exact} is both local and external"
            ),
            Self::MissingSourceOwner(owner) => write!(
                f,
                "foundation source representation {owner} has no nominal snapshot"
            ),
            Self::RepresentationAccess(owner) => write!(
                f,
                "foundation source representation {owner} has mismatched access"
            ),
            Self::Identity(error) => write!(f, "invalid foundation source identity: {error}"),
        }
    }
}

impl std::error::Error for TypeFoundationSourceError {}
