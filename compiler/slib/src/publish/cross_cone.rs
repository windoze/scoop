//! Artifact summaries and atomic persistence for cross-Cone production.

use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;

use super::{CompileViewSummaryV1, LinkViewSummaryV1};
use crate::{ArtifactFingerprint, CanonicalSlibArchive, DependencyRecord};

mod atomic;
mod layout;
pub use layout::*;

/// Manifest, dependency, and object information retained for normal consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeArtifactSummary {
    manifest: crate::ArtifactManifestSummaryV1,
    compile_summary: CompileViewSummaryV1,
    link_summary: LinkViewSummaryV1,
}

impl CrossConeArtifactSummary {
    pub const fn manifest_summary(&self) -> &crate::ArtifactManifestSummaryV1 {
        &self.manifest
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.manifest.artifact_fingerprint()
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.manifest.cone().coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.manifest.cone().identity()
    }

    pub const fn kind(&self) -> crate::ConeKind {
        self.manifest.cone().kind()
    }

    pub const fn source_form(&self) -> crate::ConeSourceForm {
        self.manifest.cone().source_form()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.manifest.target_selection()
    }

    pub const fn profile(&self) -> &ArtifactCapabilityProfileId {
        self.manifest.profile()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.manifest.direct_dependencies()
    }

    pub fn dependency_record(&self) -> DependencyRecord {
        self.manifest.dependency_record()
    }

    pub const fn compile_summary(&self) -> CompileViewSummaryV1 {
        self.compile_summary
    }

    pub const fn link_summary(&self) -> &LinkViewSummaryV1 {
        &self.link_summary
    }
}

#[derive(Debug)]
pub struct PublishedCrossConeArtifact {
    path: PathBuf,
    summary: CrossConeArtifactSummary,
}

impl PublishedCrossConeArtifact {
    pub(crate) fn write(
        archive: &CanonicalSlibArchive,
        summary: &CrossConeArtifactSummary,
        destination: &Path,
    ) -> Result<Self, CrossConeArtifactPublishError> {
        atomic::publish(archive.as_bytes(), destination, summary.clone())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn summary(&self) -> &CrossConeArtifactSummary {
        &self.summary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConePublishIoOperation {
    CreateTemporary,
    WriteTemporary,
    RenameTemporary,
}

impl fmt::Display for CrossConePublishIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateTemporary => "create private publication file",
            Self::WriteTemporary => "write private publication file",
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
        }
    }
}

impl std::error::Error for CrossConeArtifactPublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::MissingParent { .. } => None,
        }
    }
}
