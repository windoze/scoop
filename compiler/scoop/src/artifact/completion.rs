use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use scoop_identity::ConeIdentity;
use scoop_protocol::StructuredDiagnosticV1;
use scoop_slib::{ArtifactFingerprint, DependencyRecord};

use super::{
    ArtifactClosurePlan, ArtifactClosureValidationError, BuildArtifact, ValidatedArtifactClosure,
};
use crate::PreparedArtifactCandidate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletedNodeOrigin {
    Prebuilt,
    CacheHit,
    Compiled,
}

/// A completed node retains its artifact data and exact dependency closure.
#[derive(Clone, Debug)]
pub struct CompletedNode {
    cone: ConeIdentity,
    origin: CompletedNodeOrigin,
    artifact: Rc<BuildArtifact>,
    closure: Rc<ValidatedArtifactClosure>,
    artifact_locator: PathBuf,
    warnings: Vec<StructuredDiagnosticV1>,
    used_generators: Vec<scoop_protocol::GeneratorToolInputV1>,
    environment_inputs: Vec<scoop_protocol::EnvironmentInputV1>,
}

impl CompletedNode {
    pub(crate) fn with_used_generators(
        mut self,
        inputs: Vec<scoop_protocol::GeneratorToolInputV1>,
    ) -> Self {
        self.used_generators = inputs;
        self
    }

    pub(crate) fn with_environment_inputs(
        mut self,
        inputs: Vec<scoop_protocol::EnvironmentInputV1>,
    ) -> Self {
        self.environment_inputs = inputs;
        self
    }

    pub(crate) fn environment_inputs(&self) -> &[scoop_protocol::EnvironmentInputV1] {
        &self.environment_inputs
    }

    pub(crate) fn used_generators(&self) -> &[scoop_protocol::GeneratorToolInputV1] {
        &self.used_generators
    }
    pub const fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub const fn origin(&self) -> CompletedNodeOrigin {
        self.origin
    }

    pub fn artifact(&self) -> &BuildArtifact {
        &self.artifact
    }

    pub fn closure(&self) -> &ValidatedArtifactClosure {
        &self.closure
    }

    /// The original prebuilt file or the published cache output.
    pub fn artifact_locator(&self) -> &Path {
        &self.artifact_locator
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub(crate) fn shared_artifact(&self) -> Rc<BuildArtifact> {
        Rc::clone(&self.artifact)
    }

    pub(crate) fn from_result(
        cone: ConeIdentity,
        origin: CompletedNodeOrigin,
        artifact: Rc<BuildArtifact>,
        closures: ValidatedArtifactClosure,
        artifact_locator: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        Self {
            cone,
            origin,
            artifact,
            closure: Rc::new(closures),
            artifact_locator,
            warnings,
            used_generators: Vec::new(),
            environment_inputs: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum PrebuiltCompletionError {
    NotPrebuilt(ConeIdentity),
    EmptyCandidateSet(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    CurrentNodeAlreadyCompleted(ConeIdentity),
    CandidatePlan {
        path: PathBuf,
        source: Box<ArtifactClosureValidationError>,
    },
    StalePrebuiltDependency {
        path: PathBuf,
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        completed: Box<DependencyRecord>,
    },
    CandidateDisagreement {
        first: PathBuf,
        second: PathBuf,
    },
}

impl fmt::Display for PrebuiltCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPrebuilt(identity) => {
                write!(formatter, "Cone {identity} is not a prepared prebuilt node")
            }
            Self::EmptyCandidateSet(identity) => {
                write!(
                    formatter,
                    "prepared prebuilt Cone {identity} has no candidates"
                )
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::CurrentNodeAlreadyCompleted(identity) => write!(
                formatter,
                "prebuilt Cone {identity} was supplied as its own completed dependency"
            ),
            Self::CandidatePlan { path, source } => write!(
                formatter,
                "prebuilt candidate {} does not match the resolved graph: {source}",
                path.display()
            ),
            Self::StalePrebuiltDependency {
                path,
                dependent,
                dependency,
                ..
            } => write!(
                formatter,
                "prebuilt candidate {} for {dependent} records stale semantic fingerprints for dependency {dependency}",
                path.display()
            ),
            Self::CandidateDisagreement { first, second } => write!(
                formatter,
                "prebuilt candidates {} and {} have different artifact fingerprints",
                first.display(),
                second.display()
            ),
        }
    }
}

impl std::error::Error for PrebuiltCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CandidatePlan { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct ValidatedCandidate {
    source_locator: PathBuf,
    artifact: Rc<BuildArtifact>,
    closures: ValidatedArtifactClosure,
    agreement: ArtifactFingerprint,
}

pub(crate) fn complete_prebuilt_candidates(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    candidates: Vec<PreparedArtifactCandidate>,
    completed: &[&CompletedNode],
) -> Result<CompletedNode, PrebuiltCompletionError> {
    let mut artifacts = BTreeMap::new();
    for node in completed {
        if node.cone == identity {
            return Err(PrebuiltCompletionError::CurrentNodeAlreadyCompleted(
                identity,
            ));
        }
        if artifacts
            .insert(node.cone, node.shared_artifact())
            .is_some()
        {
            return Err(PrebuiltCompletionError::DuplicateCompletedNode(node.cone));
        }
    }

    let mut validated = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let artifact = candidate.shared_artifact();
        artifacts.insert(identity, Rc::clone(&artifact));
        let closures = plan
            .validate(identity, &artifacts)
            .map_err(|source| match source {
                ArtifactClosureValidationError::StaleDependency {
                    dependent,
                    dependency,
                    recorded,
                    completed,
                } => PrebuiltCompletionError::StalePrebuiltDependency {
                    path: candidate.source_locator().to_path_buf(),
                    dependent,
                    dependency,
                    recorded,
                    completed,
                },
                source => PrebuiltCompletionError::CandidatePlan {
                    path: candidate.source_locator().to_path_buf(),
                    source: Box::new(source),
                },
            })?;
        let summary = artifact.summary();
        validated.push(ValidatedCandidate {
            source_locator: candidate.source_locator().to_path_buf(),
            agreement: summary.artifact_fingerprint(),
            artifact,
            closures,
        });
    }

    let first = validated
        .first()
        .ok_or(PrebuiltCompletionError::EmptyCandidateSet(identity))?;
    if let Some(disagreeing) = validated
        .iter()
        .skip(1)
        .find(|candidate| candidate.agreement != first.agreement)
    {
        return Err(PrebuiltCompletionError::CandidateDisagreement {
            first: first.source_locator.clone(),
            second: disagreeing.source_locator.clone(),
        });
    }
    let selected = validated
        .into_iter()
        .min_by(|left, right| left.source_locator.cmp(&right.source_locator))
        .ok_or(PrebuiltCompletionError::EmptyCandidateSet(identity))?;
    Ok(CompletedNode {
        cone: identity,
        origin: CompletedNodeOrigin::Prebuilt,
        artifact: selected.artifact,
        closure: Rc::new(selected.closures),
        artifact_locator: selected.source_locator,
        warnings: Vec::new(),
        used_generators: Vec::new(),
        environment_inputs: Vec::new(),
    })
}
