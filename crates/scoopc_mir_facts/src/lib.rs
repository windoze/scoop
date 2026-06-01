//! MIR fact product shared by later compiler stages.
//!
//! This crate is intentionally data-only: it depends on the stage-independent
//! base crates for identity, source, span, type, and cone context primitives, and
//! it does not depend on the `scoopc` facade, HIR/MIR nodes, effect stages, LIR,
//! or codegen ABI types.

#![forbid(unsafe_code)]

pub mod backend;
pub mod boundary;
pub mod common;
pub mod dump;
pub mod effects;
pub mod families;
pub mod metadata;
pub mod pass_artifacts;
pub mod pipeline;
pub mod provenance;
pub mod roots;
pub mod snapshot;
pub mod verify;

use backend::MirBackendFacts;
use boundary::MirBoundaryFacts;
use effects::MirEffectFacts;
use families::InstanceFamilyInventory;
use metadata::MirMetadataFacts;
use pass_artifacts::PassArtifactMetadata;
use pipeline::MirPassPipelineMetadata;
use provenance::MirProvenanceFacts;
use roots::RootInventories;
use snapshot::SnapshotBindings;

/// Complete set of MIR-owned facts published by the MIR stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirFacts {
    pub schema_version: scoopc_types::WireSchemaVersion,
    pub roots: RootInventories,
    pub snapshots: SnapshotBindings,
    pub families: InstanceFamilyInventory,
    pub effects: MirEffectFacts,
    pub provenance: MirProvenanceFacts,
    pub boundary: MirBoundaryFacts,
    pub backend: MirBackendFacts,
    pub pass_artifacts: PassArtifactMetadata,
    pub pass_pipeline: MirPassPipelineMetadata,
    pub metadata: MirMetadataFacts,
}

impl Default for MirFacts {
    fn default() -> Self {
        Self {
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            roots: RootInventories::default(),
            snapshots: SnapshotBindings::default(),
            families: InstanceFamilyInventory::default(),
            effects: MirEffectFacts::default(),
            provenance: MirProvenanceFacts::default(),
            boundary: MirBoundaryFacts::default(),
            backend: MirBackendFacts::default(),
            pass_artifacts: PassArtifactMetadata::default(),
            pass_pipeline: MirPassPipelineMetadata::default(),
            metadata: MirMetadataFacts::default(),
        }
    }
}

impl MirFacts {
    /// Create an empty fact product for tests and staged construction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether all fact groups are currently empty.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
            && self.snapshots.is_empty()
            && self.families.is_empty()
            && self.effects.is_empty()
            && self.provenance.is_empty()
            && self.boundary.is_empty()
            && self.backend.is_empty()
            && self.pass_artifacts.is_empty()
            && self.pass_pipeline.is_empty()
            && self.metadata.is_empty()
    }

    /// Verify structural invariants before handing facts to later stages.
    pub fn verify(&self) -> verify::Result<()> {
        verify::verify_mir_facts(self)
    }

    /// Render a stable textual summary of the fact groups.
    pub fn dump(&self) -> String {
        dump::dump_mir_facts(self)
    }
}

#[cfg(test)]
mod tests {
    use scoop_project_model::{OptLevel, StableConeKey};
    use scoopc_ids::{CanonicalTextKey, StableCanonicalKey as _, StageArtifactKey};

    use super::*;
    use crate::common::FactIdentity;
    use crate::metadata::{MirNominalOwnerKind, NominalDirectSupertypesFact};
    use crate::roots::{MirItemReference, MirRootDetail, MirRootFact, MirRootKind};
    use crate::snapshot::MaterializedSnapshotBinding;
    use crate::verify::VerifyError;

    #[test]
    fn empty_mir_facts_verify_and_dump_group_boundaries() {
        let facts = MirFacts::new();

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("roots: callable_bodies=0"));
        assert!(dump.contains("snapshots: canonical=<none>"));
        assert!(dump.contains("pass_pipeline: runs=0"));
    }

    #[test]
    fn mir_facts_bincode_round_trip_preserves_schema_and_content() {
        let facts = MirFacts::new();
        let bytes = bincode::serialize(&facts).expect("serialize MIR facts");
        let decoded: MirFacts = bincode::deserialize(&bytes).expect("deserialize MIR facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
    }

    #[test]
    fn verifier_rejects_duplicate_fact_identities() {
        let duplicate = root_fact("app.main", MirRootKind::CallableBody);
        let mut facts = MirFacts::new();
        facts.roots.callable_bodies.push(duplicate.clone());
        facts.roots.callable_bodies.push(duplicate);

        let err = facts.verify().unwrap_err();
        assert_eq!(
            err,
            VerifyError::DuplicateFactIdentity("app.main".to_string())
        );
    }

    #[test]
    fn verifier_checks_canonical_snapshot_binding() {
        let cone = StableConeKey::new("fixture", "0.0.0");
        let key = StageArtifactKey::new("mir", &cone, "snapshot", 1);
        let mut facts = MirFacts::new();
        facts.snapshots.canonical = Some(key.clone());

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MissingCanonicalSnapshot(key.canonical_text())
        );

        facts
            .snapshots
            .snapshots
            .push(MaterializedSnapshotBinding::new(
                key,
                cone,
                OptLevel::O0,
                0,
                1,
            ));
        assert!(facts.verify().is_ok());
    }

    #[test]
    fn metadata_facts_publish_nominal_direct_supertypes() {
        let mut facts = MirFacts::new();
        facts
            .metadata
            .nominal_direct_supertypes
            .push(NominalDirectSupertypesFact::new(
                identity("mir_metadata:nominal_direct_supertypes:app.Derived"),
                MirNominalOwnerKind::Nominal,
                "app.Derived",
                vec!["app.Base".to_string()],
            ));

        assert_eq!(
            facts.metadata.direct_supertypes("app.Derived"),
            Some(["app.Base".to_string()].as_slice())
        );
        assert!(facts.verify().is_ok());
    }

    fn root_fact(key: &str, kind: MirRootKind) -> MirRootFact {
        MirRootFact::new(
            identity(key),
            kind,
            key,
            MirItemReference::new(0),
            MirRootDetail::CallableBody,
        )
    }

    fn identity(key: &str) -> FactIdentity {
        FactIdentity::new(
            CanonicalTextKey::new(key),
            key,
            StableConeKey::new("fixture", "0.0.0"),
            None,
        )
    }
}
