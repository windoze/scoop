use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::ConeIdentity;
use scoop_protocol::{
    CurrentConeRequestV1, DiagnosticOutputPolicyV1, HostPathCarrier, HostPathError,
    ProtocolValidationError, RequestCorrelationId, ScoopcBuildRequestV1, ScoopcRequestEnvelopeV1,
    StageDumpPolicyV1, TrustedCoreRequestV1,
};

use super::{PreparedBuildGraph, PreparedGraphNode};
use crate::{ChildIoPlan, CompletedNode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChildInvocationPlanV1 {
    #[cfg(test)]
    identity: ConeIdentity,
    request: ScoopcRequestEnvelopeV1,
    output_path: PathBuf,
    io: ChildIoPlan,
}

impl ChildInvocationPlanV1 {
    #[cfg(test)]
    pub(crate) const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub(crate) const fn request(&self) -> &ScoopcRequestEnvelopeV1 {
        &self.request
    }

    pub(crate) fn observe(
        mut self,
        stages: scoop_protocol::StageDumpSet,
    ) -> Result<Self, ChildRequestPlanError> {
        self.request = self
            .request
            .observe(stages)
            .map_err(ChildRequestPlanError::Protocol)?;
        Ok(self)
    }

    pub(crate) fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub(crate) const fn io(&self) -> &ChildIoPlan {
        &self.io
    }
}

impl PreparedBuildGraph {
    /// Constructs the only machine request shape accepted for one prepared
    /// source node. Every artifact path comes from an already completed node
    /// in the dependency graph.
    pub(crate) fn child_invocation_plan(
        &self,
        identity: ConeIdentity,
        request_id: RequestCorrelationId,
        completed: &[&CompletedNode],
    ) -> Result<ChildInvocationPlanV1, ChildRequestPlanError> {
        self.child_output_plan(identity, request_id, completed, None)
    }

    pub(crate) fn child_documentation_plan(
        &self,
        identity: ConeIdentity,
        request_id: RequestCorrelationId,
        completed: &[&CompletedNode],
        path: &Path,
        artifact: bool,
    ) -> Result<ChildInvocationPlanV1, ChildRequestPlanError> {
        self.child_output_plan(identity, request_id, completed, Some((path, artifact)))
    }

    fn child_output_plan(
        &self,
        identity: ConeIdentity,
        request_id: RequestCorrelationId,
        completed: &[&CompletedNode],
        documentation: Option<(&Path, bool)>,
    ) -> Result<ChildInvocationPlanV1, ChildRequestPlanError> {
        let completed = completed_map(identity, completed)?;
        let production = if documentation.is_some_and(|(_, artifact)| !artifact) {
            None
        } else {
            Some(
                self.context
                    .target
                    .production(self.context.cache_root.as_path())
                    .map_err(ChildRequestPlanError::Toolchain)?,
            )
        };
        let target = match &production {
            Some(target) => target
                .child_request()
                .map_err(ChildRequestPlanError::Toolchain)?,
            None => scoop_protocol::TargetSelectionRequestV1::new(
                self.context.target.canonical_triple().into(),
            )
            .map_err(ChildRequestPlanError::Protocol)?,
        };
        let (current, direct, support, trusted_core, output_path) = match self.nodes.get(&identity)
        {
            Some(PreparedGraphNode::ManifestSource(node)) => {
                let projection = self.source_projection(identity)?;
                let (direct, support) = dependency_paths(identity, projection, &completed)?;
                let core = if identity == ConeIdentity::CORE {
                    TrustedCoreRequestV1::Bootstrap
                } else {
                    completed_core_input(&completed)?
                };
                (
                    CurrentConeRequestV1::ManifestRoot {
                        root: path_carrier(&node.input_root)?,
                    },
                    direct,
                    support,
                    core,
                    node.output_path.clone(),
                )
            }
            Some(PreparedGraphNode::SingleFile(node)) => {
                let projection = self.source_projection(identity)?;
                let core = completed_core_input(&completed)?;
                let (direct, support) = dependency_paths(identity, projection, &completed)?;
                (
                    CurrentConeRequestV1::SingleFile {
                        source: path_carrier(&node.input_path)?,
                    },
                    direct,
                    support,
                    core,
                    node.output_path.clone(),
                )
            }
            Some(PreparedGraphNode::PrebuiltArtifact(_)) => {
                return Err(ChildRequestPlanError::PrebuiltNode(identity));
            }
            None => return Err(ChildRequestPlanError::UnknownNode(identity)),
        };
        let native_inputs = match (production.as_ref(), self.nodes.get(&identity)) {
            (Some(_), Some(PreparedGraphNode::ManifestSource(node))) => {
                let super::SourceNativeInput::Artifact(native) = &node.snapshot.native else {
                    return Err(ChildRequestPlanError::NativeNotPrepared);
                };
                native
                    .objects()
                    .iter()
                    .map(|path| path_carrier(path))
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => Vec::new(),
        };
        let mut io = ChildIoPlan::new(
            self.context.sysroot.as_path().to_path_buf(),
            self.context.cache_root.as_path().to_path_buf(),
        );
        if let Some(target) = production {
            io = io.with_toolchain(target.c_bridge_toolchain());
        }
        let output = match documentation {
            Some((path, true)) => {
                scoop_protocol::ScoopcOutputRequestV1::ArtifactWithDocumentation {
                    artifact: path_carrier(&output_path)?,
                    documentation: path_carrier(path)?,
                }
            }
            Some((path, false)) => {
                scoop_protocol::ScoopcOutputRequestV1::Documentation(path_carrier(path)?)
            }
            None => scoop_protocol::ScoopcOutputRequestV1::Artifact(path_carrier(&output_path)?),
        };
        let build = ScoopcBuildRequestV1::new(
            current,
            direct,
            support,
            trusted_core,
            target,
            output,
            DiagnosticOutputPolicyV1::Structured,
            match self.dumps.get(&identity) {
                Some(dump) => StageDumpPolicyV1::Files {
                    stages: dump.stages,
                    directory: path_carrier(&dump.private_directory)?,
                },
                None => StageDumpPolicyV1::None,
            },
        )
        .map_err(ChildRequestPlanError::Protocol)?
        .with_optimization(self.context.optimization)
        .with_native_inputs(native_inputs)
        .with_purpose(self.compilation_purpose(identity))
        .map_err(ChildRequestPlanError::Protocol)?
        .with_generators(
            self.context
                .generators
                .registrations()
                .map_err(ChildRequestPlanError::Toolchain)?,
        )
        .map_err(ChildRequestPlanError::Protocol)?;
        Ok(ChildInvocationPlanV1 {
            #[cfg(test)]
            identity,
            request: ScoopcRequestEnvelopeV1::new(request_id, build),
            output_path,
            io,
        })
    }

    fn source_projection(
        &self,
        identity: ConeIdentity,
    ) -> Result<&crate::ResolvedDependencyProjection, ChildRequestPlanError> {
        self.source_inputs
            .get(&identity)
            .ok_or(ChildRequestPlanError::MissingSourceProjection(identity))
    }
}

fn completed_map<'a>(
    current: ConeIdentity,
    completed: &'a [&'a CompletedNode],
) -> Result<BTreeMap<ConeIdentity, &'a CompletedNode>, ChildRequestPlanError> {
    let mut by_identity = BTreeMap::new();
    for node in completed {
        if node.cone() == current {
            return Err(ChildRequestPlanError::CurrentAlreadyCompleted(current));
        }
        if by_identity.insert(node.cone(), *node).is_some() {
            return Err(ChildRequestPlanError::DuplicateCompletedNode(node.cone()));
        }
    }
    Ok(by_identity)
}

fn completed_core_input(
    completed: &BTreeMap<ConeIdentity, &CompletedNode>,
) -> Result<TrustedCoreRequestV1, ChildRequestPlanError> {
    let core = completed
        .get(&ConeIdentity::CORE)
        .ok_or(ChildRequestPlanError::MissingTrustedCore)?;
    Ok(TrustedCoreRequestV1::ArtifactSlot {
        artifact: path_carrier(core.artifact_locator())?,
    })
}

fn dependency_paths(
    current: ConeIdentity,
    projection: &crate::ResolvedDependencyProjection,
    completed: &BTreeMap<ConeIdentity, &CompletedNode>,
) -> Result<(Vec<HostPathCarrier>, Vec<HostPathCarrier>), ChildRequestPlanError> {
    let direct = artifact_paths(current, "direct", projection.direct(), completed)?;
    let support = artifact_paths(current, "support", projection.support(), completed)?;
    Ok((direct, support))
}

fn artifact_paths(
    current: ConeIdentity,
    role: &'static str,
    identities: &[ConeIdentity],
    completed: &BTreeMap<ConeIdentity, &CompletedNode>,
) -> Result<Vec<HostPathCarrier>, ChildRequestPlanError> {
    identities
        .iter()
        .map(|dependency| {
            let node =
                completed
                    .get(dependency)
                    .ok_or(ChildRequestPlanError::MissingDependency {
                        current,
                        dependency: *dependency,
                        role,
                    })?;
            path_carrier(node.artifact_locator())
        })
        .collect()
}

fn path_carrier(path: &Path) -> Result<HostPathCarrier, ChildRequestPlanError> {
    HostPathCarrier::from_path(path).map_err(|source| ChildRequestPlanError::HostPath {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug)]
pub enum ChildRequestPlanError {
    NativeNotPrepared,
    UnknownNode(ConeIdentity),
    PrebuiltNode(ConeIdentity),
    MissingSourceProjection(ConeIdentity),
    CurrentAlreadyCompleted(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    MissingTrustedCore,
    MissingDependency {
        current: ConeIdentity,
        dependency: ConeIdentity,
        role: &'static str,
    },
    HostPath {
        path: PathBuf,
        source: HostPathError,
    },
    Protocol(ProtocolValidationError),
    Toolchain(scoop_toolchain::ToolchainError),
}

impl fmt::Display for ChildRequestPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeNotPrepared => {
                formatter.write_str("documentation-only source has no native production inputs")
            }
            Self::UnknownNode(identity) => write!(formatter, "unknown prepared Cone {identity}"),
            Self::PrebuiltNode(identity) => {
                write!(
                    formatter,
                    "prebuilt Cone {identity} must not launch a compiler child"
                )
            }
            Self::MissingSourceProjection(identity) => {
                write!(
                    formatter,
                    "prepared source Cone {identity} has no dependency projection"
                )
            }
            Self::CurrentAlreadyCompleted(identity) => {
                write!(formatter, "child Cone {identity} is already completed")
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::MissingTrustedCore => {
                formatter.write_str("ordinary compiler child requires completed trusted core")
            }
            Self::MissingDependency {
                current,
                dependency,
                role,
            } => write!(
                formatter,
                "child Cone {current} is missing completed {role} dependency {dependency}"
            ),
            Self::HostPath { path, source } => {
                write!(
                    formatter,
                    "cannot transport child path {}: {source}",
                    path.display()
                )
            }
            Self::Protocol(source) => write!(formatter, "invalid child request plan: {source}"),
            Self::Toolchain(source) => {
                write!(formatter, "cannot transport compiler selection: {source}")
            }
        }
    }
}

impl std::error::Error for ChildRequestPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HostPath { source, .. } => Some(source),
            Self::Protocol(source) => Some(source),
            Self::Toolchain(source) => Some(source),
            _ => None,
        }
    }
}
