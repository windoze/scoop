//! Final two-view publication proofs for strong artifacts.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_hir::HirOutputContractV1;
use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticIdentitySession};
use scoop_lir::{
    CBridgeToolchainProfileV1, StrongExternalLirBridgeSurfaceV1, ValidatedLirTargetSelection,
};

use crate::{
    ArtifactDistributionClassV1, ArtifactFingerprint, CanonicalDefinedLinkSymbolOwnerSetV1,
    ConeKind, ConeSourceForm, DecodedSlibEnvelope, DependencyRecord, FingerprintAvailability,
    GraphValidationError, SemanticFingerprintRecord, SingleConeProductionOutputV1,
    SingleConeStrongProfile, SlibMemberId, SlibReadError, StrongCompileArtifactValidationError,
    StrongLinkArtifactValidationError, ValidatedCompileArtifact,
    ValidatedSingleConeStrongLinkArtifact,
    validate_self_describing_single_cone_strong_compile_artifact,
    validate_self_describing_single_cone_strong_link_artifact,
    validate_single_cone_strong_compile_artifact, validate_single_cone_strong_link_artifact,
};

mod cross_cone;
pub use cross_cone::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileViewSummaryV1 {
    semantic_fingerprints: SemanticFingerprintRecord,
}

impl CompileViewSummaryV1 {
    pub(crate) const fn new(semantic_fingerprints: SemanticFingerprintRecord) -> Self {
        Self {
            semantic_fingerprints,
        }
    }

    pub const fn semantic_fingerprints(self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkViewSummaryV1 {
    distribution: ArtifactDistributionClassV1,
    output: SingleConeProductionOutputV1,
    image_owner_member: SlibMemberId,
    link_object_count: usize,
    semantic_fingerprints: SemanticFingerprintRecord,
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
    profile: scoop_identity::ArtifactCapabilityProfileId,
    direct_dependencies: Vec<DependencyRecord>,
    compile_summary: CompileViewSummaryV1,
    link_summary: LinkViewSummaryV1,
}

impl PublishableSingleConeArtifact {
    pub fn from_validated_views(
        compile: &ValidatedCompileArtifact<SingleConeStrongProfile>,
        link: &ValidatedSingleConeStrongLinkArtifact<'_>,
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
        if compile.compatibility() != link.compatibility() {
            return Err(PublishViewMismatchError::Compatibility);
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
            profile: compile.compatibility().artifact_profile().clone(),
            direct_dependencies: compile.direct_dependencies().to_vec(),
            compile_summary: CompileViewSummaryV1 {
                semantic_fingerprints: compile_semantic,
            },
            link_summary: LinkViewSummaryV1 {
                distribution: manifest.distribution(),
                output: manifest.output().clone(),
                image_owner_member: manifest.image_owner_member(),
                link_object_count,
                semantic_fingerprints: link_semantic,
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

    pub const fn profile(&self) -> &scoop_identity::ArtifactCapabilityProfileId {
        &self.profile
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        &self.direct_dependencies
    }

    pub fn dependency_record(&self) -> DependencyRecord {
        let semantic = self.compile_summary.semantic_fingerprints();
        DependencyRecord::from_validated(
            self.coordinate.clone(),
            self.identity,
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
        )
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

    target_selection: ValidatedLirTargetSelection,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishableSingleConeArtifact, PublishableArtifactValidationError> {
    let compile_graph = DecodedSlibEnvelope::open(final_bytes, target_selection)
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

    let link_graph = DecodedSlibEnvelope::open(final_bytes, target_selection)
        .map_err(|error| PublishableArtifactValidationError::LinkEnvelope(Box::new(error)))?
        .validate_graph()
        .map_err(|error| PublishableArtifactValidationError::LinkGraph(Box::new(error)))?;
    let link = validate_single_cone_strong_link_artifact(
        link_graph,
        expected_external_bridges,
        dependency_owners,
        c_bridge_profile,
    )
    .map_err(|error| PublishableArtifactValidationError::Link(Box::new(error)))?;

    PublishableSingleConeArtifact::from_validated_views(&compile, &link)
        .map_err(PublishableArtifactValidationError::ViewMismatch)
}

/// Reopens final bytes through both strong views without any compiler-side IR
/// authority. External bridge expectations are reconstructed from each
/// view's validated identity graph, while trusted core definitions remain an
/// explicit caller-supplied authority.
pub fn validate_self_describing_publishable_single_cone_artifact(
    final_bytes: &[u8],

    target_selection: ValidatedLirTargetSelection,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishableSingleConeArtifact, PublishableArtifactValidationError> {
    let (compile, link) = validate_self_describing_single_cone_strong_views(
        final_bytes,
        target_selection,
        dependency_owners,
        c_bridge_profile,
    )?;

    PublishableSingleConeArtifact::from_validated_views(&compile, &link)
        .map_err(PublishableArtifactValidationError::ViewMismatch)
}

pub(crate) fn validate_self_describing_single_cone_strong_views<'input>(
    final_bytes: &'input [u8],

    target_selection: ValidatedLirTargetSelection,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<
    (
        ValidatedCompileArtifact<SingleConeStrongProfile>,
        ValidatedSingleConeStrongLinkArtifact<'input>,
    ),
    PublishableArtifactValidationError,
> {
    let compile_graph = DecodedSlibEnvelope::open(final_bytes, target_selection)
        .map_err(|error| PublishableArtifactValidationError::CompileEnvelope(Box::new(error)))?
        .validate_graph()
        .map_err(|error| PublishableArtifactValidationError::CompileGraph(Box::new(error)))?;
    let mut session = SemanticIdentitySession::new();
    let compile =
        validate_self_describing_single_cone_strong_compile_artifact(compile_graph, &mut session)
            .map_err(|error| PublishableArtifactValidationError::Compile(Box::new(error)))?;

    let link_graph = DecodedSlibEnvelope::open(final_bytes, target_selection)
        .map_err(|error| PublishableArtifactValidationError::LinkEnvelope(Box::new(error)))?
        .validate_graph()
        .map_err(|error| PublishableArtifactValidationError::LinkGraph(Box::new(error)))?;
    let link = validate_self_describing_single_cone_strong_link_artifact(
        link_graph,
        dependency_owners,
        c_bridge_profile,
    )
    .map_err(|error| PublishableArtifactValidationError::Link(Box::new(error)))?;

    Ok((compile, link))
}

/// Atomically publish exact final archive bytes after a closed-write,
/// read-only round trip through both strong views.
///
/// The caller must validate output aliasing before entering this function.
/// This function never changes an existing destination unless validation has
/// completed successfully.
#[allow(clippy::too_many_arguments)]
pub fn publish_single_cone_artifact(
    final_bytes: &[u8],
    destination: &Path,

    target_selection: ValidatedLirTargetSelection,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishedSingleConeArtifact, SingleConeArtifactPublishError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| SingleConeArtifactPublishError::MissingParent {
            destination: destination.to_path_buf(),
        })?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".scoop-publish-")
        .suffix(".slib.tmp")
        .tempfile_in(parent)
        .map_err(|source| {
            SingleConeArtifactPublishError::io(
                PublishIoOperation::CreateTemporary,
                parent.to_path_buf(),
                source,
            )
        })?;
    temporary.write_all(final_bytes).map_err(|source| {
        SingleConeArtifactPublishError::io(
            PublishIoOperation::WriteTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    temporary.flush().map_err(|source| {
        SingleConeArtifactPublishError::io(
            PublishIoOperation::FlushTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    temporary.as_file().sync_all().map_err(|source| {
        SingleConeArtifactPublishError::io(
            PublishIoOperation::SyncTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;

    // `into_temp_path` closes the writable file handle while retaining the
    // delete-on-drop guard. Validation therefore observes a fresh read-only
    // open of exactly the file that will be renamed.
    let temporary = temporary.into_temp_path();
    let round_trip_bytes = std::fs::read(&temporary).map_err(|source| {
        SingleConeArtifactPublishError::io(
            PublishIoOperation::ReadTemporary,
            temporary.to_path_buf(),
            source,
        )
    })?;
    let validation = validate_publishable_single_cone_artifact(
        &round_trip_bytes,
        target_selection,
        expected_external_bridges,
        dependency_owners,
        c_bridge_profile,
    )
    .map_err(|source| SingleConeArtifactPublishError::Validation(Box::new(source)))?;
    temporary.persist(destination).map_err(|error| {
        SingleConeArtifactPublishError::io(
            PublishIoOperation::RenameTemporary,
            destination.to_path_buf(),
            error.error,
        )
    })?;

    Ok(PublishedSingleConeArtifact {
        path: destination.to_path_buf(),
        validation,
    })
}

#[derive(Debug)]
pub struct PublishedSingleConeArtifact {
    path: PathBuf,
    validation: PublishableSingleConeArtifact,
}

impl PublishedSingleConeArtifact {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn validation(&self) -> &PublishableSingleConeArtifact {
        &self.validation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishIoOperation {
    CreateTemporary,
    WriteTemporary,
    FlushTemporary,
    SyncTemporary,
    ReadTemporary,
    RenameTemporary,
}

impl fmt::Display for PublishIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateTemporary => "create private publication file",
            Self::WriteTemporary => "write private publication file",
            Self::FlushTemporary => "flush private publication file",
            Self::SyncTemporary => "sync private publication file",
            Self::ReadTemporary => "reopen private publication file",
            Self::RenameTemporary => "atomically publish validated artifact",
        })
    }
}

#[derive(Debug)]
pub enum SingleConeArtifactPublishError {
    MissingParent {
        destination: PathBuf,
    },
    Io {
        operation: PublishIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    Validation(Box<PublishableArtifactValidationError>),
}

impl SingleConeArtifactPublishError {
    fn io(operation: PublishIoOperation, path: PathBuf, source: std::io::Error) -> Self {
        Self::Io {
            operation,
            path,
            source,
        }
    }
}

impl fmt::Display for SingleConeArtifactPublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParent { destination } => write!(
                formatter,
                "artifact destination {} has no parent directory",
                destination.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::Validation(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeArtifactPublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Validation(source) => Some(source.as_ref()),
            Self::MissingParent { .. } => None,
        }
    }
}

pub(super) fn output_matches(
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
    Compatibility,
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
    use super::*;

    #[test]
    fn final_bytes_require_independent_compile_and_link_views() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            crate::link_decode::complete_strong_artifact_identity_for_test(),
            Vec::new(),
        )
        .unwrap();
        let dependency_owners = Vec::new();
        let publishable = validate_publishable_single_cone_artifact(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &dependency_owners,
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
    fn published_bytes_reconstruct_external_bridge_authority_from_both_views() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let dependency_owners = Vec::new();

        let publishable = validate_self_describing_publishable_single_cone_artifact(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &dependency_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap();

        assert_eq!(
            publishable.identity(),
            crate::link_decode::complete_strong_artifact_identity_for_test()
        );
        assert_eq!(
            publishable.dependency_record().coordinate(),
            publishable.coordinate()
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
        let dependency_owners = Vec::new();
        let error = validate_publishable_single_cone_artifact(
            &bytes,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &dependency_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap_err();

        assert!(matches!(error, PublishableArtifactValidationError::Link(_)));
    }

    #[test]
    fn atomic_publisher_replaces_the_destination_only_after_both_views_pass() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("current.slib");
        std::fs::write(&destination, b"previous artifact").unwrap();
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            crate::link_decode::complete_strong_artifact_identity_for_test(),
            Vec::new(),
        )
        .unwrap();
        let dependency_owners = Vec::new();

        let published = publish_single_cone_artifact(
            &bytes,
            &destination,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &dependency_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap();

        assert_eq!(published.path(), destination);
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert_eq!(published.validation().identity(), external.producer());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_round_trip_preserves_the_previous_destination() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("current.slib");
        std::fs::write(&destination, b"previous artifact").unwrap();
        let bytes = crate::link_decode::complete_strong_artifact_for_test(true);
        let external = StrongExternalLirBridgeSurfaceV1::try_new(
            crate::link_decode::complete_strong_artifact_identity_for_test(),
            Vec::new(),
        )
        .unwrap();
        let dependency_owners = Vec::new();

        let error = publish_single_cone_artifact(
            &bytes,
            &destination,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &external,
            &dependency_owners,
            &crate::link_decode::c_bridge_profile_for_test(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            SingleConeArtifactPublishError::Validation(_)
        ));
        assert_eq!(std::fs::read(&destination).unwrap(), b"previous artifact");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
