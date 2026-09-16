use std::fmt;
use std::path::PathBuf;

use scoop_identity::ConeCoordinate;
use scoop_manifest::{SingleFileInputError, SourceDiscoveryError};
use scoop_slib::{PrebuiltManifestSummaryError, SlibClosureResourceErrorV1};

use super::super::staging::StagingError;
use crate::{PairedCompilerError, SnapshotFileError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreLockOperation {
    CreateDirectory,
    Inspect,
    Open,
    Lock,
}

impl fmt::Display for CoreLockOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateDirectory => "create trusted core artifact directory",
            Self::Inspect => "inspect trusted core lock",
            Self::Open => "open trusted core lock",
            Self::Lock => "lock trusted core slot",
        })
    }
}

#[derive(Debug)]
pub enum PrepareBuildGraphError {
    PairedCompiler(PairedCompilerError),
    Staging(StagingError),
    CoreLockIo {
        operation: CoreLockOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidCoreLockFileType(PathBuf),
    CoreLockPathChanged(PathBuf),
    MissingTrustedCore,
    InvalidTrustedCoreRepresentation,
    DuplicateTrustedCore,
    ManifestSnapshot(SnapshotFileError),
    ManifestChanged(PathBuf),
    SourceDiscovery(SourceDiscoveryError),
    SingleFile(SingleFileInputError),
    SourceLengthOverflow,
    ArtifactSnapshot {
        path: PathBuf,
        source: SnapshotFileError,
    },
    ArtifactLengthOverflow(PathBuf),
    ArtifactSummary {
        path: PathBuf,
        source: PrebuiltManifestSummaryError,
    },
    ArtifactSummaryChanged(PathBuf),
    PrebuiltProjectionChanged(ConeCoordinate),
    Resource(SlibClosureResourceErrorV1),
}

impl fmt::Display for PrepareBuildGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PairedCompiler(error) => error.fmt(formatter),
            Self::Staging(error) => error.fmt(formatter),
            Self::CoreLockIo {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::InvalidCoreLockFileType(path) => write!(
                formatter,
                "trusted core lock {} is not a regular file",
                path.display()
            ),
            Self::CoreLockPathChanged(path) => write!(
                formatter,
                "trusted core lock {} changed while it was acquired",
                path.display()
            ),
            Self::MissingTrustedCore => {
                formatter.write_str("resolved graph lost the trusted core node")
            }
            Self::InvalidTrustedCoreRepresentation => {
                formatter.write_str("resolved trusted core node has an invalid representation")
            }
            Self::DuplicateTrustedCore => {
                formatter.write_str("resolved graph contains duplicate trusted core nodes")
            }
            Self::ManifestSnapshot(error) => {
                write!(formatter, "cannot snapshot source manifest: {error}")
            }
            Self::ManifestChanged(path) => write!(
                formatter,
                "source manifest {} changed after graph discovery",
                path.display()
            ),
            Self::SourceDiscovery(error) => {
                write!(formatter, "cannot snapshot manifest sources: {error}")
            }
            Self::SingleFile(error) => {
                write!(formatter, "cannot snapshot single-file root: {error}")
            }
            Self::SourceLengthOverflow => {
                formatter.write_str("source snapshot length does not fit u64")
            }
            Self::ArtifactSnapshot { path, source } => write!(
                formatter,
                "cannot snapshot prebuilt artifact {}: {source}",
                path.display()
            ),
            Self::ArtifactLengthOverflow(path) => write!(
                formatter,
                "artifact snapshot length does not fit u64: {}",
                path.display()
            ),
            Self::ArtifactSummary { path, source } => write!(
                formatter,
                "cannot re-probe artifact snapshot {}: {source}",
                path.display()
            ),
            Self::ArtifactSummaryChanged(path) => write!(
                formatter,
                "artifact {} changed after graph discovery",
                path.display()
            ),
            Self::PrebuiltProjectionChanged(coordinate) => write!(
                formatter,
                "prebuilt projection for {coordinate} changed during graph preparation"
            ),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PrepareBuildGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PairedCompiler(error) => Some(error),
            Self::Staging(error) => Some(error),
            Self::CoreLockIo { source, .. } => Some(source),
            Self::ManifestSnapshot(error) => Some(error),
            Self::SourceDiscovery(error) => Some(error),
            Self::SingleFile(error) => Some(error),
            Self::ArtifactSnapshot { source, .. } => Some(source),
            Self::ArtifactSummary { source, .. } => Some(source),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
