use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity};

use scoop_wire::HashError;

use crate::discovery::EdgeOrigin;

use super::ResolvedNodeRepresentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedNodeClaim {
    pub(super) coordinate: ConeCoordinate,
    pub(super) identity: ConeIdentity,
    pub(super) incoming_origins: Vec<EdgeOrigin>,
}

impl VersionedNodeClaim {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub fn incoming_origins(&self) -> &[EdgeOrigin] {
        &self.incoming_origins
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultipleVersionConflict {
    pub(super) group: String,
    pub(super) name: String,
    pub(super) claims: Vec<VersionedNodeClaim>,
}

impl MultipleVersionConflict {
    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn claims(&self) -> &[VersionedNodeClaim] {
        &self.claims
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCycle {
    pub(super) steps: Vec<CycleStep>,
}

impl ResolvedCycle {
    pub fn steps(&self) -> &[CycleStep] {
        &self.steps
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleStep {
    pub(super) dependent: ConeIdentity,
    pub(super) dependent_coordinate: ConeCoordinate,
    pub(super) dependency: ConeIdentity,
    pub(super) dependency_coordinate: ConeCoordinate,
    pub(super) origins: Vec<EdgeOrigin>,
}

impl CycleStep {
    pub const fn dependent(&self) -> ConeIdentity {
        self.dependent
    }

    pub const fn dependent_coordinate(&self) -> &ConeCoordinate {
        &self.dependent_coordinate
    }

    pub const fn dependency(&self) -> ConeIdentity {
        self.dependency
    }

    pub const fn dependency_coordinate(&self) -> &ConeCoordinate {
        &self.dependency_coordinate
    }

    pub fn origins(&self) -> &[EdgeOrigin] {
        &self.origins
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EdgeKeyMismatch {
    pub(super) key_dependent: ConeIdentity,
    pub(super) key_dependency: ConeIdentity,
    pub(super) edge_dependent: ConeIdentity,
    pub(super) edge_dependency: ConeIdentity,
}

impl EdgeKeyMismatch {
    pub const fn key_dependent(&self) -> ConeIdentity {
        self.key_dependent
    }

    pub const fn key_dependency(&self) -> ConeIdentity {
        self.key_dependency
    }

    pub const fn edge_dependent(&self) -> ConeIdentity {
        self.edge_dependent
    }

    pub const fn edge_dependency(&self) -> ConeIdentity {
        self.edge_dependency
    }
}

#[derive(Debug)]
pub enum ResolveBuildGraphError {
    Identity(HashError),
    MissingRoot(ConeIdentity),
    MissingTrustedCore,
    NodeIdentityMismatch {
        key: ConeIdentity,
        coordinate: Box<ConeCoordinate>,
        derived: ConeIdentity,
    },
    InvalidReservedNode {
        identity: ConeIdentity,
        coordinate: Box<ConeCoordinate>,
        representation: ResolvedNodeRepresentation,
    },
    InvalidRootRepresentation {
        identity: ConeIdentity,
        representation: ResolvedNodeRepresentation,
    },
    ExecutableDependency {
        coordinate: ConeCoordinate,
    },
    MultipleVersions(Vec<MultipleVersionConflict>),
    EdgeKeyMismatch(Box<EdgeKeyMismatch>),
    MissingEdgeEndpoint {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    EdgeCoordinateMismatch {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        expected: Box<ConeCoordinate>,
        actual: Box<ConeCoordinate>,
    },
    DependencyRecordMismatch {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InvalidEdgeOrigin {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    MissingDirectCore {
        coordinate: ConeCoordinate,
    },
    InvalidSingleFileGraph,
    UnreachableNodes(Vec<ConeCoordinate>),
    Cycles(Vec<ResolvedCycle>),
    Allocation {
        purpose: &'static str,
        elements: usize,
    },
    InternalCyclePath(ConeIdentity),
    InternalCycleEdge {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InternalTopologicalState(ConeIdentity),
    InternalTopologicalLength {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ResolveBuildGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::MissingRoot(identity) => {
                write!(formatter, "resolved graph has no root {identity}")
            }
            Self::MissingTrustedCore => formatter.write_str("resolved graph has no trusted core"),
            Self::NodeIdentityMismatch {
                key,
                coordinate,
                derived,
            } => write!(
                formatter,
                "graph node {coordinate} is keyed by {key}, but derives identity {derived}"
            ),
            Self::InvalidReservedNode {
                identity,
                coordinate,
                representation,
            } => write!(
                formatter,
                "reserved graph node {coordinate} ({identity}) has invalid representation {representation:?}"
            ),
            Self::InvalidRootRepresentation {
                identity,
                representation,
            } => write!(
                formatter,
                "build root {identity} has invalid representation {representation:?}"
            ),
            Self::ExecutableDependency { coordinate } => {
                write!(formatter, "non-root Cone {coordinate} is executable")
            }
            Self::MultipleVersions(conflicts) => {
                formatter.write_str("resolved graph contains multiple exact versions")?;
                for conflict in conflicts {
                    write!(formatter, "; {}:{} =", conflict.group, conflict.name)?;
                    for claim in &conflict.claims {
                        write!(formatter, " {}", claim.coordinate.version())?;
                    }
                }
                Ok(())
            }
            Self::EdgeKeyMismatch(_) => {
                formatter.write_str("resolved graph edge key does not match its endpoints")
            }
            Self::MissingEdgeEndpoint {
                dependent,
                dependency,
            } => write!(
                formatter,
                "dependency edge {dependent} -> {dependency} has a missing endpoint"
            ),
            Self::EdgeCoordinateMismatch {
                dependent,
                expected,
                actual,
                ..
            } => write!(
                formatter,
                "dependency edge from {dependent} expects {expected}, but records {actual}"
            ),
            Self::DependencyRecordMismatch {
                dependent,
                dependency,
            } => write!(
                formatter,
                "artifact dependency record {dependent} -> {dependency} does not match the resolved edge"
            ),
            Self::InvalidEdgeOrigin {
                dependent,
                dependency,
            } => write!(
                formatter,
                "dependency edge {dependent} -> {dependency} has an invalid origin or expectation"
            ),
            Self::MissingDirectCore { coordinate } => {
                write!(
                    formatter,
                    "Cone {coordinate} has no direct trusted-core edge"
                )
            }
            Self::InvalidSingleFileGraph => formatter.write_str(
                "single-file graph must contain exactly one synthetic edge to trusted core",
            ),
            Self::UnreachableNodes(coordinates) => {
                formatter.write_str("resolved graph contains unreachable nodes")?;
                for coordinate in coordinates {
                    write!(formatter, "; {coordinate}")?;
                }
                Ok(())
            }
            Self::Cycles(cycles) => {
                formatter.write_str("resolved graph contains dependency cycles")?;
                for cycle in cycles {
                    formatter.write_str("; ")?;
                    if let Some(first) = cycle.steps.first() {
                        write!(formatter, "{}", first.dependent_coordinate)?;
                        for step in &cycle.steps {
                            write!(formatter, " -> {}", step.dependency_coordinate)?;
                        }
                    }
                }
                Ok(())
            }
            Self::Allocation { purpose, elements } => write!(
                formatter,
                "could not allocate {elements} elements for {purpose}"
            ),
            Self::InternalCyclePath(identity) => {
                write!(formatter, "could not construct cycle path from {identity}")
            }
            Self::InternalCycleEdge {
                dependent,
                dependency,
            } => write!(
                formatter,
                "cycle path references missing edge {dependent} -> {dependency}"
            ),
            Self::InternalTopologicalState(identity) => {
                write!(formatter, "invalid topological state for {identity}")
            }
            Self::InternalTopologicalLength { expected, actual } => write!(
                formatter,
                "topological order contains {actual} nodes, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ResolveBuildGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            _ => None,
        }
    }
}
