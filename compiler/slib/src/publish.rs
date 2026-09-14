//! Final two-view publication proof for one strong single-Cone artifact.

use std::fmt;

use scoop_hir::HirOutputContractV1;
use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticIdentitySession};
use scoop_lir::{
    CBridgeToolchainProfileV1, StrongExternalLirBridgeSurfaceV1, ValidatedLirTargetSelection,
};
use scoop_wire::{DecodeLimits, DecodeUsage};

use crate::{
    ArtifactDistributionClassV1, ArtifactFingerprint, CanonicalDefinedLinkSymbolOwnerSetV1,
    ConeKind, ConeSourceForm, DecodedSlibEnvelope, FingerprintAvailability, GraphValidationError,
    SemanticFingerprintRecord, SingleConeProductionOutputV1, SingleConeStrongProfile, SlibMemberId,
    SlibReadError, StrongCompileArtifactValidationError, StrongLinkArtifactValidationError,
    ValidatedCompileArtifact, ValidatedSingleConeStrongLinkArtifact,
    validate_single_cone_strong_compile_artifact, validate_single_cone_strong_link_artifact,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileViewSummaryV1 {
    semantic_fingerprints: SemanticFingerprintRecord,
    decode_usage: DecodeUsage,
}

impl CompileViewSummaryV1 {
    pub const fn semantic_fingerprints(self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }

    pub const fn decode_usage(self) -> DecodeUsage {
        self.decode_usage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkViewSummaryV1 {
    distribution: ArtifactDistributionClassV1,
    output: SingleConeProductionOutputV1,
    image_owner_member: SlibMemberId,
    link_object_count: usize,
    semantic_fingerprints: SemanticFingerprintRecord,
    decode_usage: DecodeUsage,
}

impl LinkViewSummaryV1 {
    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.distribution
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        &self.output
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.image_owner_member
    }

    pub const fn link_object_count(&self) -> usize {
        self.link_object_count
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.decode_usage
    }
}

/// Immutable authority that the exact final archive bytes independently pass
/// both strong Compile and Link validation.
#[derive(Debug)]
pub struct PublishableSingleConeArtifact {
    artifact_fingerprint: ArtifactFingerprint,
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
    kind: ConeKind,
    source_form: ConeSourceForm,
    target_selection: ValidatedLirTargetSelection,
    compile_summary: CompileViewSummaryV1,
    link_summary: LinkViewSummaryV1,
}

impl PublishableSingleConeArtifact {
    pub fn from_validated_views(
        compile: ValidatedCompileArtifact<'_, SingleConeStrongProfile>,
        link: ValidatedSingleConeStrongLinkArtifact<'_>,
    ) -> Result<Self, PublishViewMismatchError> {
        if compile.artifact_fingerprint() != link.artifact_fingerprint() {
            return Err(PublishViewMismatchError::ArtifactFingerprint);
        }
        if compile.identity() != link.identity() || compile.coordinate() != link.coordinate() {
            return Err(PublishViewMismatchError::Cone);
        }
        if compile.kind() != link.kind() || compile.source_form() != link.source_form() {
            return Err(PublishViewMismatchError::ConeShape);
        }
        if compile.target_selection() != link.target_selection() {
            return Err(PublishViewMismatchError::TargetSelection);
        }

        let compile_semantic = compile.semantic_fingerprints();
        let link_semantic = link.semantic_fingerprints();
        if compile_semantic != link_semantic {
            return Err(PublishViewMismatchError::SemanticFingerprints);
        }
        let manifest = link.production_manifest();
        if compile_semantic.code()
            != FingerprintAvailability::Available(manifest.code_fingerprint())
        {
            return Err(PublishViewMismatchError::CodeFingerprint);
        }
        if compile_semantic.runtime_image()
            != FingerprintAvailability::Available(manifest.runtime_image_fingerprint())
        {
            return Err(PublishViewMismatchError::RuntimeImageFingerprint);
        }
        if !output_matches(
            compile.kind(),
            compile.production().hir().output_contract(),
            manifest.output(),
        ) {
            return Err(PublishViewMismatchError::Output);
        }

        let link_object_count = manifest
            .code_proof()
            .production()
            .link_objects()
            .members()
            .len();
        if link_object_count == 0 {
            return Err(PublishViewMismatchError::EmptyLinkObjectSet);
        }

        Ok(Self {
            artifact_fingerprint: compile.artifact_fingerprint(),
            coordinate: compile.coordinate().clone(),
            identity: compile.identity(),
            kind: compile.kind(),
            source_form: compile.source_form(),
            target_selection: compile.target_selection(),
            compile_summary: CompileViewSummaryV1 {
                semantic_fingerprints: compile_semantic,
                decode_usage: compile.decode_usage(),
            },
            link_summary: LinkViewSummaryV1 {
                distribution: manifest.distribution(),
                output: manifest.output().clone(),
                image_owner_member: manifest.image_owner_member(),
                link_object_count,
                semantic_fingerprints: link_semantic,
                decode_usage: link.decode_usage(),
            },
        })
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn kind(&self) -> ConeKind {
        self.kind
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.source_form
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn compile_summary(&self) -> CompileViewSummaryV1 {
        self.compile_summary
    }

    pub const fn link_summary(&self) -> &LinkViewSummaryV1 {
        &self.link_summary
    }
}

/// Reopen the same final byte slice twice and construct the only publication
/// proof accepted by the strong single-Cone writer.
pub fn validate_publishable_single_cone_artifact(
    final_bytes: &[u8],
    limits: DecodeLimits,
    target_selection: ValidatedLirTargetSelection,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishableSingleConeArtifact, PublishableArtifactValidationError> {
    let compile_graph = DecodedSlibEnvelope::open(final_bytes, limits, target_selection)
        .map_err(|error| PublishableArtifactValidationError::CompileEnvelope(Box::new(error)))?
        .validate_graph()
        .map_err(|error| PublishableArtifactValidationError::CompileGraph(Box::new(error)))?;
    let mut session = SemanticIdentitySession::new();
    let compile = validate_single_cone_strong_compile_artifact(
        compile_graph,
        expected_external_bridges,
        &mut session,
    )
    .map_err(|error| PublishableArtifactValidationError::Compile(Box::new(error)))?;

    let link_graph = DecodedSlibEnvelope::open(final_bytes, limits, target_selection)
        .map_err(|error| PublishableArtifactValidationError::LinkEnvelope(Box::new(error)))?
        .validate_graph()
        .map_err(|error| PublishableArtifactValidationError::LinkGraph(Box::new(error)))?;
    let link = validate_single_cone_strong_link_artifact(
        link_graph,
        expected_external_bridges,
        core_owners,
        c_bridge_profile,
    )
    .map_err(|error| PublishableArtifactValidationError::Link(Box::new(error)))?;

    PublishableSingleConeArtifact::from_validated_views(compile, link)
        .map_err(PublishableArtifactValidationError::ViewMismatch)
}

fn output_matches(
    kind: ConeKind,
    compile: &HirOutputContractV1,
    link: &SingleConeProductionOutputV1,
) -> bool {
    matches!(
        (kind, compile, link),
        (
            ConeKind::Library,
            HirOutputContractV1::Library,
            SingleConeProductionOutputV1::Library
        ) | (
            ConeKind::Executable,
            HirOutputContractV1::Executable(_),
            SingleConeProductionOutputV1::Executable(_)
        )
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishViewMismatchError {
    ArtifactFingerprint,
    Cone,
    ConeShape,
    TargetSelection,
    SemanticFingerprints,
    CodeFingerprint,
    RuntimeImageFingerprint,
    Output,
    EmptyLinkObjectSet,
}

impl fmt::Display for PublishViewMismatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Compile/Link publication views disagree: {self:?}"
        )
    }
}

impl std::error::Error for PublishViewMismatchError {}

#[derive(Debug)]
pub enum PublishableArtifactValidationError {
    CompileEnvelope(Box<SlibReadError>),
    CompileGraph(Box<GraphValidationError>),
    Compile(Box<StrongCompileArtifactValidationError>),
    LinkEnvelope(Box<SlibReadError>),
    LinkGraph(Box<GraphValidationError>),
    Link(Box<StrongLinkArtifactValidationError>),
    ViewMismatch(PublishViewMismatchError),
}

impl fmt::Display for PublishableArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "artifact is not publishable: {self:?}")
    }
}

impl std::error::Error for PublishableArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::CompileEnvelope(error) | Self::LinkEnvelope(error) => error.as_ref(),
            Self::CompileGraph(error) | Self::LinkGraph(error) => error.as_ref(),
            Self::Compile(error) => error.as_ref(),
            Self::Link(error) => error.as_ref(),
            Self::ViewMismatch(error) => error,
        })
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::ConeIdentity;

    use super::*;

    #[test]
    fn final_bytes_require_independent_compile_and_link_views() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            crate::link_decode::complete_strong_artifact_identity_for_test(),
            Vec::new(),
        )
        .unwrap();
        let core_owners = CanonicalDefinedLinkSymbolOwnerSetV1::empty_for_test(ConeIdentity::CORE);
        let publishable = validate_publishable_single_cone_artifact(
            &bytes,
            DecodeLimits::default(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &core_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap();

        assert_eq!(publishable.identity(), external.producer());
        assert_eq!(publishable.kind(), ConeKind::Library);
        assert_eq!(publishable.link_summary().link_object_count(), 1);
        assert_eq!(
            publishable.compile_summary().semantic_fingerprints(),
            publishable.link_summary().semantic_fingerprints()
        );
    }

    #[test]
    fn corrupt_final_object_cannot_gain_publication_authority() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            crate::link_decode::complete_strong_artifact_identity_for_test(),
            Vec::new(),
        )
        .unwrap();
        let core_owners = CanonicalDefinedLinkSymbolOwnerSetV1::empty_for_test(ConeIdentity::CORE);
        let error = validate_publishable_single_cone_artifact(
            &bytes,
            DecodeLimits::default(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &core_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap_err();

        assert!(matches!(error, PublishableArtifactValidationError::Link(_)));
    }
}
