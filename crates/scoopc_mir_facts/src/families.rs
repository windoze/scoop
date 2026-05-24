//! Instance and callable-family inventories derived by the MIR stage.

use scoopc_ids::{CanonicalTextKey, StageArtifactKey};
use scoopc_types::TypeId;

use crate::common::{FactIdentity, MirBodyReference};

/// MIR-owned inventory for materialized instances and callable families.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstanceFamilyInventory {
    pub instances: Vec<InstanceInventoryEntry>,
    pub callable_families: Vec<CallableFamilyFact>,
}

impl InstanceFamilyInventory {
    /// Return whether no instance or family facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty() && self.callable_families.is_empty()
    }
}

/// Stable materialized-instance entry that does not expose MIR-internal keys.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstanceInventoryEntry {
    pub identity: FactIdentity,
    pub artifact: StageArtifactKey,
    pub callable: CanonicalTextKey,
    pub type_args: Vec<TypeId>,
    pub body: Option<MirBodyReference>,
}

impl InstanceInventoryEntry {
    /// Create an instance inventory entry keyed by a stable stage artifact.
    pub fn new(
        identity: FactIdentity,
        artifact: StageArtifactKey,
        callable: CanonicalTextKey,
        type_args: Vec<TypeId>,
        body: Option<MirBodyReference>,
    ) -> Self {
        Self {
            identity,
            artifact,
            callable,
            type_args,
            body,
        }
    }
}

/// Stable callable-family entry that does not expose MIR-internal keys.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallableFamilyFact {
    pub identity: FactIdentity,
    pub callable: CanonicalTextKey,
    pub canonical_body: Option<MirBodyReference>,
    pub instances: Vec<StageArtifactKey>,
}

impl CallableFamilyFact {
    /// Create a callable-family fact from stable callable and instance keys.
    pub fn new(
        identity: FactIdentity,
        callable: CanonicalTextKey,
        canonical_body: Option<MirBodyReference>,
        instances: Vec<StageArtifactKey>,
    ) -> Self {
        Self {
            identity,
            callable,
            canonical_body,
            instances,
        }
    }
}
