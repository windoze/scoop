use std::{fmt, path::PathBuf};

use scoop_identity::ConeCoordinate;
use scoop_manifest::{DependencyCoordinateKey, ManifestRootError};
use scoop_slib::{ArtifactFingerprint, ArtifactManifestSummaryError, ConeSourceForm};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocatorIoOperation {
    Inspect,
    Canonicalize,
    Open,
    Read,
    Write,
    Lock,
}

impl fmt::Display for LocatorIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::Canonicalize => "canonicalize",
            Self::Open => "open",
            Self::Read => "read",
            Self::Write => "write",
            Self::Lock => "lock",
        })
    }
}

#[derive(Debug)]
pub enum DependencyLocatorError {
    UndeclaredDependency(DependencyCoordinateKey),
    MissingLocatorProjection(DependencyCoordinateKey),
    Manifest(ManifestRootError),
    Io {
        operation: LocatorIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    Summary {
        path: PathBuf,
        source: ArtifactManifestSummaryError,
    },

    CoordinateMismatch {
        expected: Box<ConeCoordinate>,
        actual: Box<ConeCoordinate>,
        path: PathBuf,
    },
    ExecutableDependency {
        coordinate: ConeCoordinate,
        path: PathBuf,
    },
    InvalidArtifactSourceForm {
        coordinate: ConeCoordinate,
        path: PathBuf,
        actual: ConeSourceForm,
    },
    ReservedArtifact {
        coordinate: ConeCoordinate,
        path: PathBuf,
    },
    SelfSourceLocator {
        coordinate: ConeCoordinate,
        root: PathBuf,
    },
    ArtifactNotFound {
        coordinate: ConeCoordinate,
        checked: Vec<PathBuf>,
    },
    SysrootArtifactNotFound {
        coordinate: ConeCoordinate,
        target: &'static str,
        path: PathBuf,
    },
    AmbiguousArtifact {
        coordinate: ConeCoordinate,
        candidates: Vec<(PathBuf, ArtifactFingerprint)>,
    },
}

impl fmt::Display for DependencyLocatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndeclaredDependency(key) => {
                write!(formatter, "manifest does not declare dependency {key}")
            }
            Self::MissingLocatorProjection(key) => {
                write!(
                    formatter,
                    "manifest dependency {key} has no locator projection"
                )
            }
            Self::Manifest(error) => error.fmt(formatter),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::Summary { path, source } => {
                write!(
                    formatter,
                    "invalid artifact summary {}: {source}",
                    path.display()
                )
            }

            Self::CoordinateMismatch {
                expected,
                actual,
                path,
            } => write!(
                formatter,
                "dependency at {} declares {actual}, expected {expected}",
                path.display()
            ),
            Self::ExecutableDependency { coordinate, path } => write!(
                formatter,
                "dependency {coordinate} at {} is executable; dependencies must be libraries",
                path.display()
            ),
            Self::InvalidArtifactSourceForm {
                coordinate,
                path,
                actual,
            } => write!(
                formatter,
                "dependency {coordinate} at {} has invalid source form {actual:?}",
                path.display()
            ),
            Self::ReservedArtifact { coordinate, path } => write!(
                formatter,
                "dependency {coordinate} at {} uses a reserved artifact identity",
                path.display()
            ),
            Self::SelfSourceLocator { coordinate, root } => write!(
                formatter,
                "dependency {coordinate} resolves to its declaring source root {}",
                root.display()
            ),
            Self::ArtifactNotFound {
                coordinate,
                checked,
            } => write!(
                formatter,
                "artifact {coordinate} was not found in {} checked search roots",
                checked.len()
            ),
            Self::SysrootArtifactNotFound {
                coordinate,
                target,
                path,
            } => write!(
                formatter,
                "sysroot artifact {coordinate} for {target} was not found at {}",
                path.display()
            ),
            Self::AmbiguousArtifact {
                coordinate,
                candidates,
            } => write!(
                formatter,
                "artifact {coordinate} has {} candidates with different fingerprints",
                candidates.len()
            ),
        }
    }
}

impl std::error::Error for DependencyLocatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Summary { source, .. } => Some(source),

            _ => None,
        }
    }
}
