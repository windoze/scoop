//! Owned immutable artifact backing with purpose-preserving re-open proofs.

use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;

use scoop_identity::{ArtifactCapabilityProfileId, SemanticIdentitySession};
use scoop_lir::{CBridgeToolchainProfileV1, ValidatedLirTargetSelection};
use scoop_wire::{DecodeLimits, Digest256, sha256};

use crate::{
    ArtifactFingerprint, CanonicalDefinedLinkSymbolOwnerSetV1, CompileViewSummaryV1,
    DecodedSlibEnvelope, GraphValidationError, LinkViewSummaryV1, PrebuiltManifestSummaryError,
    PublishableArtifactValidationError, PublishableSingleConeArtifact, SingleConeStrongProfile,
    SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1, SlibClosureResourceErrorV1,
    SlibReadError, StrongCompileArtifactValidationError, StrongLinkArtifactValidationError,
    ValidatedCompileArtifact, ValidatedSingleConeStrongLinkArtifact,
    probe_prebuilt_manifest_summary, validate_self_describing_single_cone_strong_compile_artifact,
    validate_self_describing_single_cone_strong_link_artifact,
    validate_self_describing_single_cone_strong_views,
};

mod purpose;

pub use purpose::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSnapshot {
    bytes: Arc<[u8]>,
    digest: Digest256,
}

impl ArtifactSnapshot {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::from_shared(bytes.into())
    }

    pub fn from_shared(bytes: Arc<[u8]>) -> Self {
        let digest = sha256(&bytes);
        Self { bytes, digest }
    }

    pub const fn digest(&self) -> Digest256 {
        self.digest
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// Returns the immutable snapshot bytes. Possessing these bytes grants no
    /// Graph, Compile, Link, or publication authority.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn probe_prebuilt_summary(
        &self,
        limits: DecodeLimits,
        target: ValidatedLirTargetSelection,
    ) -> Result<crate::PrebuiltManifestSummaryV1, PrebuiltManifestSummaryError> {
        probe_prebuilt_manifest_summary(self.as_bytes(), limits, target)
    }

    pub fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(self.as_bytes())
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileViewCertificateV1 {
    artifact: ArtifactFingerprint,
    snapshot: Digest256,
    target: ValidatedLirTargetSelection,
    profile: ArtifactCapabilityProfileId,
    summary: CompileViewSummaryV1,
}

impl CompileViewCertificateV1 {
    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact
    }

    pub const fn snapshot_digest(&self) -> Digest256 {
        self.snapshot
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn profile(&self) -> &ArtifactCapabilityProfileId {
        &self.profile
    }

    pub const fn summary(&self) -> CompileViewSummaryV1 {
        self.summary
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkViewCertificateV1 {
    artifact: ArtifactFingerprint,
    snapshot: Digest256,
    target: ValidatedLirTargetSelection,
    profile: ArtifactCapabilityProfileId,
    summary: LinkViewSummaryV1,
}

impl LinkViewCertificateV1 {
    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact
    }

    pub const fn snapshot_digest(&self) -> Digest256 {
        self.snapshot
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn profile(&self) -> &ArtifactCapabilityProfileId {
        &self.profile
    }

    pub const fn summary(&self) -> &LinkViewSummaryV1 {
        &self.summary
    }
}

#[derive(Debug)]
pub struct DualValidatedArtifactHandle {
    snapshot: Arc<ArtifactSnapshot>,
    publication: PublishableSingleConeArtifact,
    compile_certificate: CompileViewCertificateV1,
    link_certificate: LinkViewCertificateV1,
    limits: DecodeLimits,
    core_owners: CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: CBridgeToolchainProfileV1,
}

impl DualValidatedArtifactHandle {
    pub fn validate(
        snapshot: Arc<ArtifactSnapshot>,
        limits: DecodeLimits,
        target: ValidatedLirTargetSelection,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
        c_bridge_profile: &CBridgeToolchainProfileV1,
        closure_meter: &mut SlibClosureDecodeMeterV1,
    ) -> Result<Self, DualValidatedArtifactError> {
        let summary = probe_prebuilt_manifest_summary(snapshot.bytes(), limits, target)
            .map_err(DualValidatedArtifactError::Summary)?;
        closure_meter
            .observe_artifact_snapshot(&summary, snapshot.digest())
            .map_err(DualValidatedArtifactError::Resource)?;
        closure_meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::GraphSummary,
                summary.artifact_fingerprint(),
                snapshot.digest(),
                summary.decode_usage(),
            )
            .map_err(DualValidatedArtifactError::Resource)?;

        let (compile, link) = validate_self_describing_single_cone_strong_views(
            snapshot.bytes(),
            limits,
            target,
            core_owners,
            c_bridge_profile,
        )
        .map_err(DualValidatedArtifactError::Views)?;
        let publication = PublishableSingleConeArtifact::from_validated_views(&compile, &link)
            .map_err(|error| {
                DualValidatedArtifactError::Views(PublishableArtifactValidationError::ViewMismatch(
                    error,
                ))
            })?;
        verify_summary(&summary, &publication)?;

        closure_meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Compile,
                publication.artifact_fingerprint(),
                snapshot.digest(),
                compile.decode_usage(),
            )
            .map_err(DualValidatedArtifactError::Resource)?;
        closure_meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Link,
                publication.artifact_fingerprint(),
                snapshot.digest(),
                link.decode_usage(),
            )
            .map_err(DualValidatedArtifactError::Resource)?;

        let compile_certificate = CompileViewCertificateV1 {
            artifact: publication.artifact_fingerprint(),
            snapshot: snapshot.digest(),
            target,
            profile: publication.profile().clone(),
            summary: publication.compile_summary(),
        };
        let link_certificate = LinkViewCertificateV1 {
            artifact: publication.artifact_fingerprint(),
            snapshot: snapshot.digest(),
            target,
            profile: publication.profile().clone(),
            summary: publication.link_summary().clone(),
        };
        Ok(Self {
            snapshot,
            publication,
            compile_certificate,
            link_certificate,
            limits,
            core_owners: core_owners.clone(),
            c_bridge_profile: c_bridge_profile.clone(),
        })
    }

    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        &self.snapshot
    }

    pub const fn publication(&self) -> &PublishableSingleConeArtifact {
        &self.publication
    }

    pub const fn compile_certificate(&self) -> &CompileViewCertificateV1 {
        &self.compile_certificate
    }

    pub const fn link_certificate(&self) -> &LinkViewCertificateV1 {
        &self.link_certificate
    }

    pub fn with_compile_view<R>(
        &self,
        use_view: impl for<'view> FnOnce(&ValidatedCompileArtifact<'view, SingleConeStrongProfile>) -> R,
    ) -> Result<R, DualValidatedArtifactReopenError> {
        let graph = DecodedSlibEnvelope::open(
            self.snapshot.bytes(),
            self.limits,
            self.compile_certificate.target,
        )
        .map_err(DualValidatedArtifactReopenError::CompileEnvelope)?
        .validate_graph()
        .map_err(DualValidatedArtifactReopenError::CompileGraph)?;
        let mut session = SemanticIdentitySession::new();
        let view =
            validate_self_describing_single_cone_strong_compile_artifact(graph, &mut session)
                .map_err(|error| DualValidatedArtifactReopenError::Compile(Box::new(error)))?;
        self.verify_compile_certificate(&view)?;
        Ok(use_view(&view))
    }

    pub fn with_link_view<R>(
        &self,
        use_view: impl for<'view> FnOnce(&ValidatedSingleConeStrongLinkArtifact<'view>) -> R,
    ) -> Result<R, DualValidatedArtifactReopenError> {
        let graph = DecodedSlibEnvelope::open(
            self.snapshot.bytes(),
            self.limits,
            self.link_certificate.target,
        )
        .map_err(DualValidatedArtifactReopenError::LinkEnvelope)?
        .validate_graph()
        .map_err(DualValidatedArtifactReopenError::LinkGraph)?;
        let view = validate_self_describing_single_cone_strong_link_artifact(
            graph,
            &self.core_owners,
            &self.c_bridge_profile,
        )
        .map_err(|error| DualValidatedArtifactReopenError::Link(Box::new(error)))?;
        self.verify_link_certificate(&view)?;
        Ok(use_view(&view))
    }

    fn verify_compile_certificate(
        &self,
        view: &ValidatedCompileArtifact<'_, SingleConeStrongProfile>,
    ) -> Result<(), DualValidatedArtifactReopenError> {
        let candidate = CompileViewCertificateV1 {
            artifact: view.artifact_fingerprint(),
            snapshot: self.snapshot.digest(),
            target: view.target_selection(),
            profile: view.compatibility().artifact_profile().clone(),
            summary: CompileViewSummaryV1::new(view.semantic_fingerprints(), view.decode_usage()),
        };
        if candidate != self.compile_certificate
            || !common_view_matches(
                &self.publication,
                view.coordinate(),
                view.identity(),
                view.kind(),
                view.source_form(),
                view.direct_dependencies(),
                view.compatibility().artifact_profile(),
            )
        {
            return Err(DualValidatedArtifactReopenError::CertificateMismatch(
                ArtifactViewPurposeV1::Compile,
            ));
        }
        Ok(())
    }

    fn verify_link_certificate(
        &self,
        view: &ValidatedSingleConeStrongLinkArtifact<'_>,
    ) -> Result<(), DualValidatedArtifactReopenError> {
        if view.artifact_fingerprint() != self.link_certificate.artifact
            || view.target_selection() != self.link_certificate.target
            || self.snapshot.digest() != self.link_certificate.snapshot
            || view.compatibility().artifact_profile() != self.link_certificate.profile()
            || view.semantic_fingerprints() != self.link_certificate.summary.semantic_fingerprints()
            || view.decode_usage() != self.link_certificate.summary.decode_usage()
            || !common_view_matches(
                &self.publication,
                view.coordinate(),
                view.identity(),
                view.kind(),
                view.source_form(),
                view.direct_dependencies(),
                view.compatibility().artifact_profile(),
            )
        {
            return Err(DualValidatedArtifactReopenError::CertificateMismatch(
                ArtifactViewPurposeV1::Link,
            ));
        }
        Ok(())
    }
}

fn verify_summary(
    summary: &crate::PrebuiltManifestSummaryV1,
    publication: &PublishableSingleConeArtifact,
) -> Result<(), DualValidatedArtifactError> {
    if summary.cone().coordinate() != publication.coordinate()
        || summary.cone().identity() != publication.identity()
        || summary.cone().kind() != publication.kind()
        || summary.cone().source_form() != publication.source_form()
        || summary.target_selection() != publication.target_selection()
        || summary.artifact_fingerprint() != publication.artifact_fingerprint()
        || summary.profile() != publication.profile()
        || summary.direct_dependencies() != publication.direct_dependencies()
        || summary.semantic_fingerprints() != publication.compile_summary().semantic_fingerprints()
    {
        return Err(DualValidatedArtifactError::SummaryViewMismatch);
    }
    Ok(())
}

fn common_view_matches(
    publication: &PublishableSingleConeArtifact,
    coordinate: &scoop_identity::ConeCoordinate,
    identity: scoop_identity::ConeIdentity,
    kind: crate::ConeKind,
    source_form: crate::ConeSourceForm,
    dependencies: &[crate::DependencyRecord],
    profile: &ArtifactCapabilityProfileId,
) -> bool {
    publication.coordinate() == coordinate
        && publication.identity() == identity
        && publication.kind() == kind
        && publication.source_form() == source_form
        && publication.direct_dependencies() == dependencies
        && publication.link_summary().semantic_fingerprints()
            == publication.compile_summary().semantic_fingerprints()
        && profile == publication.profile()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactViewPurposeV1 {
    Compile,
    Link,
}

#[derive(Debug)]
pub enum DualValidatedArtifactError {
    Summary(PrebuiltManifestSummaryError),
    Resource(SlibClosureResourceErrorV1),
    Views(PublishableArtifactValidationError),
    SummaryViewMismatch,
}

impl fmt::Display for DualValidatedArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Summary(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Views(error) => error.fmt(formatter),
            Self::SummaryViewMismatch => {
                formatter.write_str("prebuilt summary and full artifact views disagree")
            }
        }
    }
}

impl std::error::Error for DualValidatedArtifactError {}

#[derive(Debug)]
pub enum DualValidatedArtifactReopenError {
    CompileEnvelope(SlibReadError),
    CompileGraph(GraphValidationError),
    Compile(Box<StrongCompileArtifactValidationError>),
    LinkEnvelope(SlibReadError),
    LinkGraph(GraphValidationError),
    Link(Box<StrongLinkArtifactValidationError>),
    CertificateMismatch(ArtifactViewPurposeV1),
}

impl fmt::Display for DualValidatedArtifactReopenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompileEnvelope(error) | Self::LinkEnvelope(error) => error.fmt(formatter),
            Self::CompileGraph(error) | Self::LinkGraph(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::Link(error) => error.fmt(formatter),
            Self::CertificateMismatch(purpose) => {
                write!(
                    formatter,
                    "reopened {purpose:?} view disagrees with its certificate"
                )
            }
        }
    }
}

impl std::error::Error for DualValidatedArtifactReopenError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_snapshot_can_be_reprobed_and_materialized_without_authority() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let snapshot = ArtifactSnapshot::from_bytes(bytes.clone());
        let summary = snapshot
            .probe_prebuilt_summary(
                DecodeLimits::default(),
                ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            )
            .unwrap();
        let mut materialized = Vec::new();
        snapshot.write_to(&mut materialized).unwrap();

        assert_eq!(materialized, bytes);
        assert_eq!(summary.artifact_fingerprint().as_array().len(), 32);
    }

    #[test]
    fn immutable_snapshot_retains_independent_compile_and_link_certificates() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let snapshot = Arc::new(ArtifactSnapshot::from_bytes(bytes.clone()));
        let mut meter =
            SlibClosureDecodeMeterV1::new(crate::SlibClosureDecodeLimitsV1::M23_DEFAULT);
        let handle = DualValidatedArtifactHandle::validate(
            Arc::clone(&snapshot),
            DecodeLimits::default(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap(),
            &crate::link_decode::c_bridge_profile_for_test(),
            &mut meter,
        )
        .unwrap();

        assert_eq!(snapshot.byte_length(), bytes.len());
        assert_eq!(
            handle.compile_certificate().snapshot_digest(),
            snapshot.digest()
        );
        assert_eq!(
            handle.link_certificate().snapshot_digest(),
            snapshot.digest()
        );
        let compile_identity = handle.with_compile_view(|view| view.identity()).unwrap();
        let link_identity = handle.with_link_view(|view| view.identity()).unwrap();
        assert_eq!(compile_identity, link_identity);
        assert_eq!(compile_identity, handle.publication().identity());
        assert!(meter.usage().artifact_snapshot_bytes > 0);
        assert!(meter.usage().validation_work_units > 0);
    }

    #[test]
    fn link_semantic_corruption_cannot_construct_a_dual_handle() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let snapshot = Arc::new(ArtifactSnapshot::from_bytes(bytes));
        let mut meter =
            SlibClosureDecodeMeterV1::new(crate::SlibClosureDecodeLimitsV1::M23_DEFAULT);
        let error = DualValidatedArtifactHandle::validate(
            snapshot,
            DecodeLimits::default(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap(),
            &crate::link_decode::c_bridge_profile_for_test(),
            &mut meter,
        )
        .unwrap_err();
        assert!(matches!(error, DualValidatedArtifactError::Views(_)));
    }
}
