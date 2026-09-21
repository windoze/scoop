//! Canonical dependency-first serial execution for one prepared build graph.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_protocol::{RequestCorrelationId, StructuredDiagnosticV1};
use scoop_slib::{CompileArtifactPurpose, ConeKind, LinkArtifactPurpose};

use crate::{
    CompileCacheKeyError, ConeCompileCacheKeyV1, OrdinarySourceExecutionError,
    PrebuiltCompletionError, PreparedBuildGraph, PreparedNodeRepresentation,
    ProductionSingleConeCompilerRunner, SingleConeCompilerRunner, ValidatedArtifactClosure,
};
use crate::{CompletedNode, CompletedNodeOrigin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildNodeObservation {
    identity: ConeIdentity,
    origin: CompletedNodeOrigin,
    cache_key: Option<ConeCompileCacheKeyV1>,
}

impl BuildNodeObservation {
    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn origin(&self) -> CompletedNodeOrigin {
        self.origin
    }

    pub const fn cache_key(&self) -> Option<ConeCompileCacheKeyV1> {
        self.cache_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildObservations {
    nodes: Vec<BuildNodeObservation>,
    child_invocations: Vec<ConeIdentity>,
}

impl BuildObservations {
    pub fn nodes(&self) -> &[BuildNodeObservation] {
        &self.nodes
    }

    pub fn child_invocations(&self) -> &[ConeIdentity] {
        &self.child_invocations
    }
}

#[derive(Debug)]
pub struct ExecutedBuildGraph {
    _prepared: PreparedBuildGraph,
    root: ConeIdentity,
    dependency_first: Vec<ConeIdentity>,
    completed: BTreeMap<ConeIdentity, CompletedNode>,
    observations: BuildObservations,
}

impl ExecutedBuildGraph {
    pub const fn root_identity(&self) -> ConeIdentity {
        self.root
    }

    pub fn dependency_first(&self) -> &[ConeIdentity] {
        &self.dependency_first
    }

    pub fn completed(&self, identity: ConeIdentity) -> Option<&CompletedNode> {
        self.completed.get(&identity)
    }

    pub const fn observations(&self) -> &BuildObservations {
        &self.observations
    }

    pub fn into_outcome(self) -> BuildGraphOutcome {
        let root = self.completed[&self.root].clone();
        let compile = root.compile_closure().clone();
        let link = root.link_closure().clone();
        let warnings = self
            .dependency_first
            .iter()
            .flat_map(|identity| self.completed[identity].warnings().iter().cloned())
            .collect();
        match root.artifact().publication().kind() {
            ConeKind::Library => BuildGraphOutcome::Library {
                root,
                compile,
                link,
                warnings,
                observations: self.observations,
            },
            ConeKind::Executable => BuildGraphOutcome::ExecutableArtifact {
                root,
                compile,
                link,
                warnings,
                observations: self.observations,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum BuildGraphOutcome {
    Library {
        root: CompletedNode,
        compile: ValidatedArtifactClosure<CompileArtifactPurpose>,
        link: ValidatedArtifactClosure<LinkArtifactPurpose>,
        warnings: Vec<StructuredDiagnosticV1>,
        observations: BuildObservations,
    },
    ExecutableArtifact {
        root: CompletedNode,
        compile: ValidatedArtifactClosure<CompileArtifactPurpose>,
        link: ValidatedArtifactClosure<LinkArtifactPurpose>,
        warnings: Vec<StructuredDiagnosticV1>,
        observations: BuildObservations,
    },
}

impl BuildGraphOutcome {
    pub const fn root(&self) -> &CompletedNode {
        match self {
            Self::Library { root, .. } | Self::ExecutableArtifact { root, .. } => root,
        }
    }

    pub const fn compile(&self) -> &ValidatedArtifactClosure<CompileArtifactPurpose> {
        match self {
            Self::Library { compile, .. } | Self::ExecutableArtifact { compile, .. } => compile,
        }
    }

    pub const fn link(&self) -> &ValidatedArtifactClosure<LinkArtifactPurpose> {
        match self {
            Self::Library { link, .. } | Self::ExecutableArtifact { link, .. } => link,
        }
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        match self {
            Self::Library { warnings, .. } | Self::ExecutableArtifact { warnings, .. } => warnings,
        }
    }

    pub const fn observations(&self) -> &BuildObservations {
        match self {
            Self::Library { observations, .. } | Self::ExecutableArtifact { observations, .. } => {
                observations
            }
        }
    }
}

impl PreparedBuildGraph {
    pub fn execute(self) -> Result<ExecutedBuildGraph, BuildGraphExecutionError> {
        self.execute_with_runner(&mut ProductionSingleConeCompilerRunner)
    }

    pub(crate) fn execute_with_runner(
        mut self,
        runner: &mut impl SingleConeCompilerRunner,
    ) -> Result<ExecutedBuildGraph, BuildGraphExecutionError> {
        let order = self.dependency_first().to_vec();
        let mut completed = BTreeMap::new();
        let mut observations = Vec::with_capacity(order.len());
        let mut child_invocations = Vec::new();

        for (position, identity) in order.iter().copied().enumerate() {
            let completed_refs = order[..position]
                .iter()
                .map(|completed_identity| &completed[completed_identity])
                .collect::<Vec<_>>();
            let representation = self
                .node_representation(identity)
                .ok_or(BuildGraphExecutionError::MissingPreparedNode(identity))?;
            let cache_key = match representation {
                PreparedNodeRepresentation::ManifestSource
                | PreparedNodeRepresentation::SingleFile => Some(
                    self.compile_cache_key(identity, &completed_refs)
                        .map_err(|source| {
                            BuildGraphExecutionError::CacheKey(identity, Box::new(source))
                        })?,
                ),
                PreparedNodeRepresentation::PrebuiltArtifact => None,
            };
            let request_id = request_id(position)?;
            let node = match representation {
                PreparedNodeRepresentation::PrebuiltArtifact => self
                    .complete_prebuilt_node(identity, &completed_refs)
                    .map_err(|source| {
                        BuildGraphExecutionError::Prebuilt(identity, Box::new(source))
                    })?,
                PreparedNodeRepresentation::ManifestSource
                | PreparedNodeRepresentation::SingleFile => {
                    let node = self
                        .execute_ordinary_source(identity, &completed_refs, runner, request_id)
                        .map_err(|source| {
                            BuildGraphExecutionError::Ordinary(identity, Box::new(source))
                        })?;
                    if node.origin() == CompletedNodeOrigin::Compiled {
                        child_invocations.push(identity);
                    }
                    node
                }
            };
            observations.push(BuildNodeObservation {
                identity,
                origin: node.origin(),
                cache_key,
            });
            completed.insert(identity, node);
        }

        let root = self.root_identity();
        Ok(ExecutedBuildGraph {
            _prepared: self,
            root,
            dependency_first: order,
            completed,
            observations: BuildObservations {
                nodes: observations,
                child_invocations,
            },
        })
    }
}

fn request_id(position: usize) -> Result<RequestCorrelationId, BuildGraphExecutionError> {
    let ordinal = u128::try_from(position)
        .ok()
        .and_then(|position| position.checked_add(1))
        .ok_or(BuildGraphExecutionError::RequestIdOverflow)?;
    Ok(RequestCorrelationId::from_array(ordinal.to_be_bytes()))
}

#[derive(Debug)]
pub enum BuildGraphExecutionError {
    MissingPreparedNode(ConeIdentity),
    RequestIdOverflow,
    CacheKey(ConeIdentity, Box<CompileCacheKeyError>),
    Prebuilt(ConeIdentity, Box<PrebuiltCompletionError>),
    Ordinary(ConeIdentity, Box<OrdinarySourceExecutionError>),
}

impl fmt::Display for BuildGraphExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPreparedNode(identity) => {
                write!(
                    formatter,
                    "canonical schedule names missing Cone {identity}"
                )
            }
            Self::RequestIdOverflow => {
                formatter.write_str("child request ordinal does not fit u128")
            }
            Self::CacheKey(identity, source) => {
                write!(
                    formatter,
                    "cannot derive cache key for {identity}: {source}"
                )
            }
            Self::Prebuilt(identity, source) => {
                write!(
                    formatter,
                    "cannot complete prebuilt Cone {identity}: {source}"
                )
            }
            Self::Ordinary(identity, source) => {
                write!(formatter, "cannot execute source Cone {identity}: {source}")
            }
        }
    }
}

impl std::error::Error for BuildGraphExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CacheKey(_, source) => Some(source.as_ref()),
            Self::Prebuilt(_, source) => Some(source.as_ref()),
            Self::Ordinary(_, source) => Some(source.as_ref()),
            Self::MissingPreparedNode(_) | Self::RequestIdOverflow => None,
        }
    }
}
