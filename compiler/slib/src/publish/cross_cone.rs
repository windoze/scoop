//! Publication proof and atomic persistence for the cross-Cone strong profile.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_identity::{
    ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity, SemanticIdentitySession,
};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::DecodeLimits;

use super::{CompileViewSummaryV1, LinkViewSummaryV1, PublishViewMismatchError, output_matches};
use crate::{
    ArtifactFingerprint, CrossConeArtifactClosureValidationError,
    CrossConeLinkSemanticImportBuildError, CrossConeLinkSemanticImportSetV1,
    CrossConeSemanticsStrongProfile, DependencyRecord, FingerprintAvailability,
    ValidatedCompileArtifact, ValidatedCrossConeStrongLinkArtifact,
    validate_completed_cross_cone_artifact_closure,
};

/// Immutable summary proving that one exact final archive passed the M23-5
/// Compile and Link views, including their byte-exact dependency-import
/// projection equality.
#[derive(Debug)]
pub struct PublishableCrossConeArtifact {
    artifact_fingerprint: ArtifactFingerprint,
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
    kind: crate::ConeKind,
    source_form: crate::ConeSourceForm,
    target_selection: ValidatedLirTargetSelection,
    profile: ArtifactCapabilityProfileId,
    direct_dependencies: Vec<DependencyRecord>,
    compile_summary: CompileViewSummaryV1,
    link_summary: LinkViewSummaryV1,
}

impl PublishableCrossConeArtifact {
    pub fn from_validated_views(
        compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
        link: &ValidatedCrossConeStrongLinkArtifact<'_>,
    ) -> Result<Self, CrossConePublishViewMismatchError> {
        validate_common_views(compile, link)?;
        validate_semantic_import_projection(
            compile.production().lir_cross_cone(),
            link.cross_cone_link_closure().semantic_imports(),
        )?;

        let compile_semantic = compile.semantic_fingerprints();
        let link_semantic = link.semantic_fingerprints();
        let manifest = link.production_manifest();
        let link_object_count = manifest
            .code_proof()
            .production()
            .link_objects()
            .members()
            .len();

        Ok(Self {
            artifact_fingerprint: compile.artifact_fingerprint(),
            coordinate: compile.coordinate().clone(),
            identity: compile.identity(),
            kind: compile.kind(),
            source_form: compile.source_form(),
            target_selection: compile.target_selection(),
            profile: compile.compatibility().artifact_profile().clone(),
            direct_dependencies: compile.direct_dependencies().to_vec(),
            compile_summary: CompileViewSummaryV1::new(compile_semantic, compile.decode_usage()),
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

    pub const fn kind(&self) -> crate::ConeKind {
        self.kind
    }

    pub const fn source_form(&self) -> crate::ConeSourceForm {
        self.source_form
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn profile(&self) -> &ArtifactCapabilityProfileId {
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

fn validate_common_views(
    compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
    link: &ValidatedCrossConeStrongLinkArtifact<'_>,
) -> Result<(), PublishViewMismatchError> {
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
    if compile_semantic.code() != FingerprintAvailability::Available(manifest.code_fingerprint()) {
        return Err(PublishViewMismatchError::CodeFingerprint);
    }
    if compile_semantic.runtime_image()
        != FingerprintAvailability::Available(manifest.runtime_image_fingerprint())
    {
        return Err(PublishViewMismatchError::RuntimeImageFingerprint);
    }
    if !output_matches(
        compile.kind(),
        compile.production().hir_core().output_contract(),
        manifest.output(),
    ) {
        return Err(PublishViewMismatchError::Output);
    }
    if manifest
        .code_proof()
        .production()
        .link_objects()
        .members()
        .is_empty()
    {
        return Err(PublishViewMismatchError::EmptyLinkObjectSet);
    }
    Ok(())
}

fn validate_semantic_import_projection(
    compile: &scoop_lir::CrossConeLirBridgeSectionV1,
    link: &CrossConeLinkSemanticImportSetV1,
) -> Result<(), CrossConePublishViewMismatchError> {
    let expected = CrossConeLinkSemanticImportSetV1::from_lir_bridge(compile)
        .map_err(CrossConePublishViewMismatchError::CompileProjection)?;
    if &expected != link {
        return Err(CrossConePublishViewMismatchError::SemanticImportProjection);
    }
    Ok(())
}

#[derive(Debug)]
pub enum CrossConePublishViewMismatchError {
    Common(PublishViewMismatchError),
    CompileProjection(CrossConeLinkSemanticImportBuildError),
    SemanticImportProjection,
}

impl From<PublishViewMismatchError> for CrossConePublishViewMismatchError {
    fn from(source: PublishViewMismatchError) -> Self {
        Self::Common(source)
    }
}

impl std::fmt::Display for CrossConePublishViewMismatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cross-Cone Compile/Link publication views disagree: {self:?}"
        )
    }
}

impl std::error::Error for CrossConePublishViewMismatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Common(source) => Some(source),
            Self::CompileProjection(source) => Some(source),
            Self::SemanticImportProjection => None,
        }
    }
}

/// Atomically publishes a completed artifact only after validating the exact
/// final bytes together with their full dependency closure through both views.
#[allow(clippy::too_many_arguments)]
pub fn publish_cross_cone_artifact(
    final_bytes: &[u8],
    destination: &Path,
    limits: DecodeLimits,
    current: ConeIdentity,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<&[u8]>,
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<PublishedCrossConeArtifact, CrossConeArtifactPublishError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| CrossConeArtifactPublishError::MissingParent {
            destination: destination.to_path_buf(),
        })?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".scoop-publish-")
        .suffix(".slib.tmp")
        .tempfile_in(parent)
        .map_err(|source| {
            CrossConeArtifactPublishError::io(
                CrossConePublishIoOperation::CreateTemporary,
                parent.to_path_buf(),
                source,
            )
        })?;
    temporary.write_all(final_bytes).map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::WriteTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    temporary.flush().map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::FlushTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    temporary.as_file().sync_all().map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::SyncTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;

    let temporary = temporary.into_temp_path();
    let round_trip_bytes = std::fs::read(&temporary).map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::ReadTemporary,
            temporary.to_path_buf(),
            source,
        )
    })?;
    let mut session = SemanticIdentitySession::new();
    let validation = validate_completed_cross_cone_artifact_closure(
        current,
        target,
        direct,
        dependency_first,
        &round_trip_bytes,
        limits,
        c_bridge_profile,
        &mut session,
    )
    .map_err(|source| CrossConeArtifactPublishError::Validation(Box::new(source)))?
    .into_current_publication();
    temporary.persist(destination).map_err(|error| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::RenameTemporary,
            destination.to_path_buf(),
            error.error,
        )
    })?;

    Ok(PublishedCrossConeArtifact {
        path: destination.to_path_buf(),
        validation,
    })
}

#[derive(Debug)]
pub struct PublishedCrossConeArtifact {
    path: PathBuf,
    validation: PublishableCrossConeArtifact,
}

impl PublishedCrossConeArtifact {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn validation(&self) -> &PublishableCrossConeArtifact {
        &self.validation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConePublishIoOperation {
    CreateTemporary,
    WriteTemporary,
    FlushTemporary,
    SyncTemporary,
    ReadTemporary,
    RenameTemporary,
}

impl fmt::Display for CrossConePublishIoOperation {
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
pub enum CrossConeArtifactPublishError {
    MissingParent {
        destination: PathBuf,
    },
    Io {
        operation: CrossConePublishIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    Validation(Box<CrossConeArtifactClosureValidationError>),
}

impl CrossConeArtifactPublishError {
    fn io(operation: CrossConePublishIoOperation, path: PathBuf, source: std::io::Error) -> Self {
        Self::Io {
            operation,
            path,
            source,
        }
    }
}

impl fmt::Display for CrossConeArtifactPublishError {
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

impl std::error::Error for CrossConeArtifactPublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Validation(source) => Some(source.as_ref()),
            Self::MissingParent { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_view_projection_requires_the_exact_consumer_and_import_bytes() {
        let first = empty_bridge("publication-first");
        let matching = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&first).unwrap();
        validate_semantic_import_projection(&first, &matching).unwrap();

        let second = empty_bridge("publication-second");
        let mismatched = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&second).unwrap();
        assert!(matches!(
            validate_semantic_import_projection(&first, &mismatched),
            Err(CrossConePublishViewMismatchError::SemanticImportProjection)
        ));
    }

    fn empty_bridge(name: &str) -> scoop_lir::CrossConeLirBridgeSectionV1 {
        let cone = crate::strong_compile_decode::tests::cone_named(name);
        let (foundation, _) =
            crate::link_decode::strong_production_fixture_for_test(cone.coordinate().clone());
        let foundation =
            scoop_lir::OdrFreeLirFoundation::try_new(cone.identity(), foundation).unwrap();
        scoop_lir::CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new())
            .unwrap()
    }
}
