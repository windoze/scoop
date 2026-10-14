//! Archive and manifest data used by dependency discovery, builds, and caches.
//!
//! Producers pass this data directly to build orchestration. Explicit artifact
//! readers obtain the same summary from the manifest.

use std::fmt;

use scoop_identity::ArtifactCapabilityProfileId;
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    ArtifactFingerprint, BootstrapManifest, ConeRecord, DependencyRecord, FingerprintAvailability,
    GraphValidationError, RuntimeImageFingerprint, SemanticFingerprintRecord, SlibReadError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactManifestSummaryV1 {
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    target_selection: ValidatedLirTargetSelection,
    profile: ArtifactCapabilityProfileId,
    semantic_fingerprints: SemanticFingerprintRecord,
    artifact_fingerprint: ArtifactFingerprint,
    test_directory: Option<crate::TestDirectory>,
}

mod wire;

impl ArtifactManifestSummaryV1 {
    pub fn new(
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        target_selection: ValidatedLirTargetSelection,
        profile: ArtifactCapabilityProfileId,
        semantic_fingerprints: SemanticFingerprintRecord,
        artifact_fingerprint: ArtifactFingerprint,
        test_directory: Option<crate::TestDirectory>,
    ) -> Self {
        Self {
            cone,
            direct_dependencies,
            target_selection,
            profile,
            semantic_fingerprints,
            artifact_fingerprint,
            test_directory,
        }
    }

    pub(crate) fn from_manifest(
        manifest: &BootstrapManifest,
        target_selection: ValidatedLirTargetSelection,
    ) -> Result<Self, crate::TestDirectoryError> {
        Ok(Self::new(
            manifest.cone().clone(),
            manifest.direct_dependencies().to_vec(),
            target_selection,
            manifest.compatibility().artifact_profile().clone(),
            manifest.semantic_fingerprints(),
            manifest.artifact_fingerprint(),
            crate::TestDirectory::from_manifest(manifest)?,
        ))
    }

    pub fn test_directory(&self) -> Option<&crate::TestDirectory> {
        self.test_directory.as_ref()
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        &self.direct_dependencies
    }

    pub fn dependency_record(&self) -> DependencyRecord {
        DependencyRecord::from_validated(
            self.cone.coordinate().clone(),
            self.cone.identity(),
            self.semantic_fingerprints.hir(),
            self.semantic_fingerprints.mir(),
            self.semantic_fingerprints.lir(),
        )
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn profile(&self) -> &ArtifactCapabilityProfileId {
        &self.profile
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }

    pub const fn code_fingerprint_availability(
        &self,
    ) -> FingerprintAvailability<crate::CodeFingerprint> {
        self.semantic_fingerprints.code()
    }

    pub const fn runtime_image_fingerprint_availability(
        &self,
    ) -> FingerprintAvailability<RuntimeImageFingerprint> {
        self.semantic_fingerprints.runtime_image()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }
}

pub fn read_artifact_manifest_summary(
    bytes: &[u8],

    target_selection: ValidatedLirTargetSelection,
) -> Result<ArtifactManifestSummaryV1, ArtifactManifestSummaryError> {
    let graph = crate::DecodedSlibEnvelope::open(bytes, target_selection)
        .map_err(ArtifactManifestSummaryError::Envelope)?
        .validate_graph()
        .map_err(ArtifactManifestSummaryError::Graph)?;
    let manifest = graph.envelope.manifest();
    ArtifactManifestSummaryV1::from_manifest(manifest, target_selection)
        .map_err(ArtifactManifestSummaryError::TestDirectory)
}

#[derive(Debug)]
pub enum ArtifactManifestSummaryError {
    Envelope(SlibReadError),
    Graph(GraphValidationError),
    TestDirectory(crate::TestDirectoryError),
}

impl fmt::Display for ArtifactManifestSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => error.fmt(formatter),
            Self::Graph(error) => error.fmt(formatter),
            Self::TestDirectory(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArtifactManifestSummaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Envelope(error) => Some(error),
            Self::Graph(error) => Some(error),
            Self::TestDirectory(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_retains_build_inputs() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let summary =
            read_artifact_manifest_summary(&bytes, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
                .unwrap();

        assert!(summary.test_directory().is_none());
        assert_eq!(
            summary.profile(),
            &scoop_identity::ArtifactCapabilityProfileId::single_cone_strong()
        );
        assert_eq!(
            summary.cone().identity(),
            crate::link_decode::complete_strong_artifact_identity_for_test()
        );
        assert!(matches!(
            summary.code_fingerprint_availability(),
            FingerprintAvailability::Available(_)
        ));
    }

    #[test]
    fn summary_contains_the_same_data_after_record_roundtrip() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let summary =
            read_artifact_manifest_summary(&bytes, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
                .unwrap();
        let encoded = scoop_wire::encode(&summary).unwrap();
        let decoded: ArtifactManifestSummaryV1 = scoop_wire::decode_canonical(&encoded).unwrap();
        assert_eq!(decoded, summary);
    }

    #[test]
    fn summary_rejects_member_bytes_changed_after_manifest_hashing() {
        let mut bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let last = bytes.last_mut().expect("test artifact is nonempty");
        *last ^= 0x01;
        let error =
            read_artifact_manifest_summary(&bytes, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
                .unwrap_err();
        assert!(matches!(error, ArtifactManifestSummaryError::Envelope(_)));
    }
}
