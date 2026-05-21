//! Canonical materialized MIR snapshot bindings.

use scoopc_ids::StageArtifactKey;
use scoopc_project_model::{OptLevel, StableConeKey};

/// Published MIR snapshot bindings for one cone compilation unit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotBindings {
    pub canonical: Option<StageArtifactKey>,
    pub snapshots: Vec<MaterializedSnapshotBinding>,
}

impl SnapshotBindings {
    /// Return whether no materialized snapshot binding has been published yet.
    pub fn is_empty(&self) -> bool {
        self.canonical.is_none() && self.snapshots.is_empty()
    }
}

/// Metadata that binds a canonical materialized snapshot to its cone and opt level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedSnapshotBinding {
    pub key: StageArtifactKey,
    pub cone: StableConeKey,
    pub opt_level: OptLevel,
    pub body_count: usize,
    pub revision: u32,
}

impl MaterializedSnapshotBinding {
    /// Create snapshot metadata without embedding the materialized snapshot itself.
    pub fn new(
        key: StageArtifactKey,
        cone: StableConeKey,
        opt_level: OptLevel,
        body_count: usize,
        revision: u32,
    ) -> Self {
        Self {
            key,
            cone,
            opt_level,
            body_count,
            revision,
        }
    }
}
