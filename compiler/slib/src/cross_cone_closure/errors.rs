//! Errors produced by the graph, identity, foundation, and HIR-front states.

use std::fmt;

use scoop_hir::{CoreBootstrapInterfaceValidationError, CrossConeHirInterfaceResolutionError};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, IdentityReferenceError, IdentityValidationError,
};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{ConeKind, DependencyRecord, StrongProfileFoundationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeClosureGraphError {
    CoreHasDependencyProviders,
    MissingTrustedCore,
    NonCanonicalDirectProviders {
        index: usize,
        previous: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentArtifactPresent {
        current: ConeIdentity,
    },
    CurrentArtifactIdentityMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentDirectSetMismatch {
        expected: Vec<ConeIdentity>,
        actual: Vec<ConeIdentity>,
    },
    DuplicateArtifact {
        identity: ConeIdentity,
    },
    InvalidProviderKind {
        identity: ConeIdentity,
        actual: ConeKind,
    },
    TargetMismatch {
        identity: ConeIdentity,
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    MultipleVersions {
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
    },
    MissingDirectArtifact {
        identity: ConeIdentity,
    },
    MissingDependencyArtifact {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InvalidDependencyFirstOrder {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    StaleDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        actual: Box<DependencyRecord>,
    },
    UnreachableSupport {
        identity: ConeIdentity,
    },
}

impl fmt::Display for CrossConeClosureGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone semantic closure graph: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeClosureGraphError {}

#[derive(Debug)]
pub enum CrossConeClosureIdentityError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: IdentityValidationError,
    },
}

impl fmt::Display for CrossConeClosureIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} cross-Cone identity graph slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} dependency authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid identity foundation for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureFoundationError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<StrongProfileFoundationError>,
    },
}

impl fmt::Display for CrossConeClosureFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} validated cross-Cone foundation slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid cross-Cone foundations for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureHirResolutionError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirInterfaceResolutionError<IdentityReferenceError>>,
    },
}

impl fmt::Display for CrossConeClosureHirResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} resolved cross-Cone HIR slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "cannot resolve cross-Cone HIR interface for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureHirResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureHirProductionError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: CoreBootstrapInterfaceValidationError,
    },
}

impl fmt::Display for CrossConeClosureHirProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} validated cross-Cone HIR production slots"
            ),
            Self::Artifact { identity, source } => write!(
                formatter,
                "invalid legacy HIR production surface for {identity}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeClosureHirProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } => None,
        }
    }
}
