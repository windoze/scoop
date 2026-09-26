//! Publication proofs and atomic persistence for cross-Cone strong profiles.

use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{
    ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity, SemanticIdentitySession,
};
use scoop_lir::ValidatedLirTargetSelection;

use super::{CompileViewSummaryV1, LinkViewSummaryV1};
use crate::{
    ArtifactFingerprint, CrossConeArtifactClosureValidationError, DependencyRecord,
    validate_completed_cross_cone_artifact_closure,
};

mod atomic;
mod layout;
pub use layout::*;

/// Immutable summary proving that one exact final archive passed both views
/// of its strong profile, including complete dependency-import equality.
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

/// Atomically publishes a completed artifact only after validating the exact
/// final bytes together with their full dependency closure through both views.
#[allow(clippy::too_many_arguments)]
pub fn publish_cross_cone_artifact(
    final_bytes: &[u8],
    destination: &Path,

    current: ConeIdentity,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<&[u8]>,
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<PublishedCrossConeArtifact, CrossConeArtifactPublishError> {
    atomic::publish(final_bytes, destination, |round_trip_bytes| {
        let mut session = SemanticIdentitySession::new();
        let validation = validate_completed_cross_cone_artifact_closure(
            current,
            target,
            direct,
            dependency_first,
            round_trip_bytes,
            c_bridge_profile,
            &mut session,
        )
        .map_err(|source| CrossConeArtifactPublishError::Validation(Box::new(source)))?
        .into_current_publication();
        Ok(validation)
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
    LayoutValidation(Box<CrossConeLayoutArtifactValidationError>),
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
            Self::LayoutValidation(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeArtifactPublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Validation(source) => Some(source.as_ref()),
            Self::LayoutValidation(source) => Some(source.as_ref()),
            Self::MissingParent { .. } => None,
        }
    }
}
