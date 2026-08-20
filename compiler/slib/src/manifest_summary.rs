//! Archive and manifest data used by dependency discovery, builds, and caches.
//!
//! Reading a summary checks the container, member hashes, compatibility, and
//! dependency records. Compiler consumers read typed IR and objects separately.

use std::fmt;

use scoop_identity::ArtifactCapabilityProfileId;
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    ArtifactFingerprint, BootstrapManifest, CompatibilityRecord, ConeRecord, DependencyRecord,
    FingerprintAvailability, GraphValidationError, RuntimeImageFingerprint,
    SemanticFingerprintRecord, SlibReadError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactManifestSummaryV1 {
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    compatibility: CompatibilityRecord,
    target_selection: ValidatedLirTargetSelection,
    profile: ArtifactCapabilityProfileId,
    semantic_fingerprints: SemanticFingerprintRecord,
    artifact_fingerprint: ArtifactFingerprint,
    member_count: u64,
    manifest_length: u64,
    archive_length: u64,
}

impl ArtifactManifestSummaryV1 {
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

    pub const fn compatibility(&self) -> &CompatibilityRecord {
        &self.compatibility
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

    pub const fn member_count(&self) -> u64 {
        self.member_count
    }

    pub const fn manifest_length(&self) -> u64 {
        self.manifest_length
    }

    pub const fn archive_length(&self) -> u64 {
        self.archive_length
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
    let member_count = length(manifest.members().len())?;
    let manifest_length = length(graph.envelope.manifest_length())?;
    let archive_length = length(graph.envelope.archive_length())?;
    let summary = summary_from_manifest(
        manifest,
        target_selection,
        member_count,
        manifest_length,
        archive_length,
    );
    Ok(summary)
}

fn summary_from_manifest(
    manifest: &BootstrapManifest,
    target_selection: ValidatedLirTargetSelection,
    member_count: u64,
    manifest_length: u64,
    archive_length: u64,
) -> ArtifactManifestSummaryV1 {
    ArtifactManifestSummaryV1 {
        cone: manifest.cone().clone(),
        direct_dependencies: manifest.direct_dependencies().to_vec(),
        compatibility: manifest.compatibility().clone(),
        target_selection,
        profile: manifest.compatibility().artifact_profile().clone(),
        semantic_fingerprints: manifest.semantic_fingerprints(),
        artifact_fingerprint: manifest.artifact_fingerprint(),
        member_count,
        manifest_length,
        archive_length,
    }
}

fn length(value: usize) -> Result<u64, ArtifactManifestSummaryError> {
    u64::try_from(value).map_err(|_| ArtifactManifestSummaryError::LengthOverflow)
}

#[derive(Debug)]
pub enum ArtifactManifestSummaryError {
    Envelope(SlibReadError),
    Graph(GraphValidationError),
    LengthOverflow,
}

impl fmt::Display for ArtifactManifestSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => error.fmt(formatter),
            Self::Graph(error) => error.fmt(formatter),
            Self::LengthOverflow => formatter.write_str("artifact summary length does not fit u64"),
        }
    }
}

impl std::error::Error for ArtifactManifestSummaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Envelope(error) => Some(error),
            Self::Graph(error) => Some(error),
            Self::LengthOverflow => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_retains_archive_and_manifest_fields() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let summary = read_artifact_manifest_summary(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();

        assert_eq!(summary.archive_length(), bytes.len() as u64);
        assert!(summary.manifest_length() < summary.archive_length());
        assert!(summary.member_count() > 0);
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
    fn summary_does_not_claim_link_semantic_validation() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let summary = read_artifact_manifest_summary(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        assert_eq!(summary.archive_length(), bytes.len() as u64);
    }

    #[test]
    fn summary_rejects_member_bytes_changed_after_manifest_hashing() {
        let mut bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let last = bytes.last_mut().expect("test artifact is nonempty");
        *last ^= 0x01;
        let error = read_artifact_manifest_summary(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap_err();
        assert!(matches!(error, ArtifactManifestSummaryError::Envelope(_)));
    }
}
