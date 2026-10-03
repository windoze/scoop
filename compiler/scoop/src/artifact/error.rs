use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{ArtifactFingerprint, ConeKind, ConeSourceForm, DependencyRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactPlanField {
    Coordinate,
    Kind,
    SourceForm,
    Target,
    Profile,
    ArtifactFingerprint,
}

#[derive(Debug)]
pub enum ArtifactClosureValidationError {
    UnknownRoot(ConeIdentity),
    MissingArtifact(ConeIdentity),
    IdentityMismatch {
        planned: ConeIdentity,
        actual: ConeIdentity,
    },
    CoordinateMismatch {
        identity: ConeIdentity,
        expected: Box<ConeCoordinate>,
        actual: Box<ConeCoordinate>,
    },
    KindMismatch {
        identity: ConeIdentity,
        expected: ConeKind,
        actual: ConeKind,
    },
    SourceFormMismatch {
        identity: ConeIdentity,
        expected: ConeSourceForm,
        actual: ConeSourceForm,
    },
    TargetMismatch {
        identity: ConeIdentity,
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    ProfileMismatch {
        identity: ConeIdentity,
        expected: Box<ArtifactCapabilityProfileId>,
        actual: Box<ArtifactCapabilityProfileId>,
    },
    ArtifactFingerprintMismatch {
        identity: ConeIdentity,
        expected: ArtifactFingerprint,
        actual: ArtifactFingerprint,
    },
    DuplicateDependencyRecord {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    MissingDependencyRecord {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    UnexpectedArtifactDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    DependencyCoordinateMismatch {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        expected: Box<ConeCoordinate>,
        actual: Box<ConeCoordinate>,
    },
    PlannedDependencyChanged {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        expected: Box<DependencyRecord>,
        actual: Box<DependencyRecord>,
    },
    StaleDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        completed: Box<DependencyRecord>,
    },
    MultipleVersions {
        group: String,
        name: String,
        first: String,
        second: String,
    },
    InvalidCanonicalOrder {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    EmptyCanonicalOrder(ConeIdentity),
}

impl ArtifactClosureValidationError {
    pub const fn plan_field(&self) -> Option<ArtifactPlanField> {
        match self {
            Self::CoordinateMismatch { .. } => Some(ArtifactPlanField::Coordinate),
            Self::KindMismatch { .. } => Some(ArtifactPlanField::Kind),
            Self::SourceFormMismatch { .. } => Some(ArtifactPlanField::SourceForm),
            Self::TargetMismatch { .. } => Some(ArtifactPlanField::Target),
            Self::ProfileMismatch { .. } => Some(ArtifactPlanField::Profile),
            Self::ArtifactFingerprintMismatch { .. } => {
                Some(ArtifactPlanField::ArtifactFingerprint)
            }
            _ => None,
        }
    }
}

impl fmt::Display for ArtifactClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoot(identity) => {
                write!(
                    formatter,
                    "artifact closure root {identity} is not in the build plan"
                )
            }
            Self::MissingArtifact(identity) => {
                write!(
                    formatter,
                    "artifact closure is missing completed Cone {identity}"
                )
            }
            Self::IdentityMismatch { planned, actual } => write!(
                formatter,
                "artifact identity mismatch: planned {planned}, found {actual}"
            ),
            Self::CoordinateMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} coordinate mismatch: expected {expected}, found {actual}"
            ),
            Self::KindMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} kind mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::SourceFormMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} source form mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::TargetMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} target mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::ProfileMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} profile mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::ArtifactFingerprintMismatch {
                identity,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {identity} fingerprint mismatch: expected {expected}, found {actual}"
            ),
            Self::DuplicateDependencyRecord {
                dependent,
                dependency,
            } => write!(
                formatter,
                "artifact {dependent} repeats dependency record {dependency}"
            ),
            Self::MissingDependencyRecord {
                dependent,
                dependency,
            } => write!(
                formatter,
                "artifact {dependent} is missing dependency record {dependency}"
            ),
            Self::UnexpectedArtifactDependency {
                dependent,
                dependency,
            } => write!(
                formatter,
                "artifact {dependent} declares graph-external dependency {dependency}"
            ),
            Self::DependencyCoordinateMismatch {
                dependent,
                dependency,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact {dependent} dependency {dependency} coordinate mismatch: expected {expected}, found {actual}"
            ),
            Self::PlannedDependencyChanged {
                dependent,
                dependency,
                ..
            } => write!(
                formatter,
                "artifact {dependent} dependency {dependency} no longer matches the resolved graph claim"
            ),
            Self::StaleDependency {
                dependent,
                dependency,
                ..
            } => write!(
                formatter,
                "artifact {dependent} records stale semantic fingerprints for dependency {dependency}"
            ),
            Self::MultipleVersions {
                group,
                name,
                first,
                second,
            } => write!(
                formatter,
                "artifact closure contains multiple versions of {group}:{name}: {first} and {second}"
            ),
            Self::InvalidCanonicalOrder {
                dependent,
                dependency,
            } => write!(
                formatter,
                "artifact closure order places dependent {dependent} before dependency {dependency}"
            ),
            Self::EmptyCanonicalOrder(root) => {
                write!(
                    formatter,
                    "artifact closure for root {root} has an empty order"
                )
            }
        }
    }
}

impl std::error::Error for ArtifactClosureValidationError {}
