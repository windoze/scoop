use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_slib::{
    ConeKind, ConeSourceForm, CrossConeArtifactClosureValidationError,
    PrebuiltManifestSummaryError, SlibClosureResourceErrorV1,
};
use scoop_wire::{DecodeLimits, HashError};

mod graph;
mod load;
mod validate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitDependencyRole {
    Direct,
    Support,
}

impl fmt::Display for ExplicitDependencyRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Direct => "direct",
            Self::Support => "support",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitDependencyArtifactInput {
    role: ExplicitDependencyRole,
    index: usize,
    path: PathBuf,
}

impl ExplicitDependencyArtifactInput {
    pub const fn role(&self) -> ExplicitDependencyRole {
        self.role
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for ExplicitDependencyArtifactInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} artifact {} {}",
            self.role,
            self.index,
            self.path.display()
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitDependencyLoadOperation {
    Open,
    Inspect,
    Read,
}

impl fmt::Display for ExplicitDependencyLoadOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub enum ExplicitDependencyLoadError {
    Io {
        input: ExplicitDependencyArtifactInput,
        operation: ExplicitDependencyLoadOperation,
        source: std::io::Error,
    },
    NotRegularFile(ExplicitDependencyArtifactInput),
    ArtifactTooLarge {
        input: ExplicitDependencyArtifactInput,
        actual: u64,
        limit: u64,
    },
    Resource(SlibClosureResourceErrorV1),
}

impl fmt::Display for ExplicitDependencyLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                input,
                operation,
                source,
            } => write!(formatter, "cannot {operation} dependency {input}: {source}"),
            Self::NotRegularFile(input) => {
                write!(formatter, "dependency {input} is not a regular file")
            }
            Self::ArtifactTooLarge {
                input,
                actual,
                limit,
            } => write!(
                formatter,
                "dependency {input} has {actual} bytes, exceeding the {limit}-byte input limit"
            ),
            Self::Resource(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ExplicitDependencyLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Resource(source) => Some(source),
            Self::NotRegularFile(_) | Self::ArtifactTooLarge { .. } => None,
        }
    }
}

#[derive(Debug)]
struct LoadedExplicitDependencyArtifact {
    input: ExplicitDependencyArtifactInput,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct LoadedExplicitDependencyInputs {
    artifacts: Vec<LoadedExplicitDependencyArtifact>,
    limits: DecodeLimits,
}

type DependencyValidationResult<T> = Result<T, Box<ExplicitDependencyValidationError>>;

#[derive(Debug)]
pub enum ExplicitDependencyValidationError {
    Resource(SlibClosureResourceErrorV1),
    Summary {
        input: ExplicitDependencyArtifactInput,
        source: Box<PrebuiltManifestSummaryError>,
    },
    Closure(Box<CrossConeArtifactClosureValidationError>),
    CoreInterface(crate::TrustedCoreArtifactValidationError),
    CurrentConeArtifact {
        input: ExplicitDependencyArtifactInput,
        identity: ConeIdentity,
    },
    UnsupportedArtifactShape {
        input: ExplicitDependencyArtifactInput,
        kind: ConeKind,
        source_form: ConeSourceForm,
    },
    DuplicateIdentity {
        identity: ConeIdentity,
        first: ExplicitDependencyArtifactInput,
        second: ExplicitDependencyArtifactInput,
        same_fingerprint: bool,
    },
    MultipleVersions {
        group: String,
        name: String,
        first_version: String,
        first_identity: ConeIdentity,
        second_version: String,
        second_identity: ConeIdentity,
    },
    ManifestIdentity {
        coordinate: ConeCoordinate,
        source: HashError,
    },
    ManifestDirectSet {
        declared: Vec<ConeCoordinate>,
        actual: Vec<ConeCoordinate>,
    },
    SelfDependency {
        coordinate: ConeCoordinate,
    },
    CycleToCurrentCone {
        coordinate: ConeCoordinate,
        dependency: ConeCoordinate,
    },
    MissingDependencyArtifact {
        coordinate: ConeCoordinate,
        dependency: ConeCoordinate,
    },
    DependencyRecordMismatch {
        coordinate: ConeCoordinate,
        dependency: ConeCoordinate,
    },
    DependencyCycle {
        cycle: Vec<ConeCoordinate>,
    },
    DependencyOrderState {
        identity: ConeIdentity,
    },
    DependencyOrderLength {
        expected: usize,
        actual: usize,
    },
    SupportClosure {
        missing: Vec<ConeCoordinate>,
        unexpected: Vec<ConeCoordinate>,
    },
}

impl fmt::Display for ExplicitDependencyValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Summary { input, source } => {
                write!(formatter, "cannot summarize dependency {input}: {source}")
            }
            Self::Closure(source) => {
                write!(formatter, "dependency closure is not valid: {source}")
            }
            Self::CoreInterface(source) => source.fmt(formatter),
            Self::CurrentConeArtifact { input, identity } => write!(
                formatter,
                "dependency {input} has the current Cone identity {identity}"
            ),
            Self::UnsupportedArtifactShape {
                input,
                kind,
                source_form,
            } => write!(
                formatter,
                "dependency {input} must be a manifest library, found {kind:?}/{source_form:?}"
            ),
            Self::DuplicateIdentity {
                identity,
                first,
                second,
                same_fingerprint,
            } => write!(
                formatter,
                "dependency identity {identity} appears more than once ({first}, {second}); artifact fingerprints {}",
                if *same_fingerprint { "match" } else { "differ" }
            ),
            Self::MultipleVersions {
                group,
                name,
                first_version,
                first_identity,
                second_version,
                second_identity,
            } => write!(
                formatter,
                "dependency closure contains multiple versions of {group}:{name}: {first_version} ({first_identity}) and {second_version} ({second_identity})"
            ),
            Self::ManifestIdentity { coordinate, source } => write!(
                formatter,
                "cannot derive manifest dependency identity for {coordinate}: {source}"
            ),
            Self::ManifestDirectSet { declared, actual } => write!(
                formatter,
                "manifest dependency set does not equal the direct artifact set: declared {declared:?}, actual {actual:?}"
            ),
            Self::SelfDependency { coordinate } => {
                write!(
                    formatter,
                    "dependency artifact {coordinate} depends on itself"
                )
            }
            Self::CycleToCurrentCone {
                coordinate,
                dependency,
            } => write!(
                formatter,
                "dependency artifact {coordinate} creates a cycle back to current Cone {dependency}"
            ),
            Self::MissingDependencyArtifact {
                coordinate,
                dependency,
            } => write!(
                formatter,
                "dependency artifact {coordinate} requires missing artifact {dependency}"
            ),
            Self::DependencyRecordMismatch {
                coordinate,
                dependency,
            } => write!(
                formatter,
                "dependency record from {coordinate} to {dependency} does not match the exact validated artifact"
            ),
            Self::DependencyCycle { cycle } => {
                write!(formatter, "dependency closure contains a cycle: {cycle:?}")
            }
            Self::DependencyOrderState { identity } => write!(
                formatter,
                "dependency-first ordering reached an invalid state at {identity}"
            ),
            Self::DependencyOrderLength { expected, actual } => write!(
                formatter,
                "dependency-first ordering produced {actual} nodes, expected {expected}"
            ),
            Self::SupportClosure {
                missing,
                unexpected,
            } => write!(
                formatter,
                "support artifact set is not the exact recursive closure: missing {missing:?}, unexpected {unexpected:?}"
            ),
        }
    }
}

impl std::error::Error for ExplicitDependencyValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Summary { source, .. } => Some(source.as_ref()),
            Self::Closure(source) => Some(source.as_ref()),
            Self::CoreInterface(source) => Some(source),
            Self::ManifestIdentity { source, .. } => Some(source),
            Self::CurrentConeArtifact { .. }
            | Self::UnsupportedArtifactShape { .. }
            | Self::DuplicateIdentity { .. }
            | Self::MultipleVersions { .. }
            | Self::ManifestDirectSet { .. }
            | Self::SelfDependency { .. }
            | Self::CycleToCurrentCone { .. }
            | Self::MissingDependencyArtifact { .. }
            | Self::DependencyRecordMismatch { .. }
            | Self::DependencyCycle { .. }
            | Self::DependencyOrderState { .. }
            | Self::DependencyOrderLength { .. }
            | Self::SupportClosure { .. } => None,
        }
    }
}
