use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use scoop_identity::ConeIdentity;
use scoop_lir::CBridgeToolchainProfileV1;
use scoop_protocol::StructuredDiagnosticV1;
use scoop_slib::{ArtifactFingerprint, DependencyRecord};

use super::{
    ArtifactClosurePlan, ArtifactClosureValidationError, CrossConeArtifactValidationError,
    ValidatedArtifactClosure, ValidatedCrossConeArtifactHandle,
};
use crate::{CacheCompletionError, PreparedArtifactCandidate};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletedNodeOrigin {
    Prebuilt,
    CacheHit,
    Compiled,
}

/// A transport path under the current build's private staging area. The path
/// is never used as semantic identity or artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateArtifactPath(PathBuf);

impl PrivateArtifactPath {
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) const fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

/// A completed node retains its artifact data and exact dependency closure.
#[derive(Clone, Debug)]
pub struct CompletedNode {
    cone: ConeIdentity,
    origin: CompletedNodeOrigin,
    artifact: Rc<ValidatedCrossConeArtifactHandle>,
    closure: Rc<ValidatedArtifactClosure>,
    materialized_child_path: PrivateArtifactPath,
    warnings: Vec<StructuredDiagnosticV1>,
}

impl CompletedNode {
    pub const fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub const fn origin(&self) -> CompletedNodeOrigin {
        self.origin
    }

    pub fn artifact(&self) -> &ValidatedCrossConeArtifactHandle {
        &self.artifact
    }

    pub fn closure(&self) -> &ValidatedArtifactClosure {
        &self.closure
    }

    pub const fn materialized_child_path(&self) -> &PrivateArtifactPath {
        &self.materialized_child_path
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub(crate) fn shared_artifact(&self) -> Rc<ValidatedCrossConeArtifactHandle> {
        Rc::clone(&self.artifact)
    }

    pub(crate) fn replace_warnings(&mut self, warnings: Vec<StructuredDiagnosticV1>) {
        self.warnings = warnings;
    }

    pub(crate) fn from_cache_hit(
        cone: ConeIdentity,
        artifact: Rc<ValidatedCrossConeArtifactHandle>,
        closures: ValidatedArtifactClosure,
        materialized_child_path: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        Self {
            cone,
            origin: CompletedNodeOrigin::CacheHit,
            artifact,
            closure: Rc::new(closures),
            materialized_child_path: PrivateArtifactPath::new(materialized_child_path),
            warnings,
        }
    }

    fn from_compiled(
        cone: ConeIdentity,
        artifact: Rc<ValidatedCrossConeArtifactHandle>,
        closures: ValidatedArtifactClosure,
        materialized_child_path: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        Self {
            cone,
            origin: CompletedNodeOrigin::Compiled,
            artifact,
            closure: Rc::new(closures),
            materialized_child_path: PrivateArtifactPath::new(materialized_child_path),
            warnings,
        }
    }
}

#[derive(Debug)]
pub enum CompiledCompletionError {
    CurrentNodeAlreadyCompleted(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    Artifact(Box<CrossConeArtifactValidationError>),
    Plan(Box<ArtifactClosureValidationError>),
    Warnings(Box<CacheCompletionError>),
}

impl fmt::Display for CompiledCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentNodeAlreadyCompleted(identity) => {
                write!(formatter, "compiled Cone {identity} is already completed")
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::Artifact(source) => {
                write!(
                    formatter,
                    "compiled artifact failed dual-view validation: {source}"
                )
            }
            Self::Plan(source) => {
                write!(
                    formatter,
                    "compiled artifact does not match the resolved graph: {source}"
                )
            }
            Self::Warnings(source) => {
                write!(formatter, "compiled warnings are not persistable: {source}")
            }
        }
    }
}

impl std::error::Error for CompiledCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Artifact(source) => source.as_ref(),
            Self::Plan(source) => source.as_ref(),
            Self::Warnings(source) => source.as_ref(),
            Self::CurrentNodeAlreadyCompleted(_) | Self::DuplicateCompletedNode(_) => return None,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_compiled_candidate(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    snapshot: Arc<scoop_slib::ArtifactSnapshot>,
    materialized_path: PathBuf,
    completed: &[&CompletedNode],
    warnings: Vec<StructuredDiagnosticV1>,

    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<CompletedNode, CompiledCompletionError> {
    let mut artifacts = BTreeMap::new();
    for node in completed {
        if node.cone == identity {
            return Err(CompiledCompletionError::CurrentNodeAlreadyCompleted(
                identity,
            ));
        }
        if artifacts
            .insert(node.cone, node.shared_artifact())
            .is_some()
        {
            return Err(CompiledCompletionError::DuplicateCompletedNode(node.cone));
        }
    }
    let artifact = plan
        .validate_completed_artifact(identity, snapshot, &artifacts, c_bridge_profile)
        .map_err(|source| CompiledCompletionError::Artifact(Box::new(source)))?;
    artifacts.insert(identity, Rc::clone(&artifact));
    let closures = plan
        .validate(identity, &artifacts)
        .map_err(|source| CompiledCompletionError::Plan(Box::new(source)))?;
    crate::validate_warning_origins(&warnings, closures.dependency_first())
        .map_err(|source| CompiledCompletionError::Warnings(Box::new(source)))?;
    Ok(CompletedNode::from_compiled(
        identity,
        artifact,
        closures,
        materialized_path,
        warnings,
    ))
}

#[derive(Debug)]
pub enum PrebuiltCompletionError {
    NotPrebuilt(ConeIdentity),
    EmptyCandidateSet(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    CurrentNodeAlreadyCompleted(ConeIdentity),
    CandidateArtifact {
        path: PathBuf,
        source: Box<CrossConeArtifactValidationError>,
    },
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
            Self::CandidateArtifact { path, source } => write!(
                formatter,
                "prebuilt candidate {} failed dual-view validation: {source}",
                path.display()
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
                "prebuilt candidates {} and {} disagree after full validation",
                first.display(),
                second.display()
            ),
        }
    }
}

impl std::error::Error for PrebuiltCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CandidateArtifact { source, .. } => Some(source),
            Self::CandidatePlan { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct ValidatedCandidate {
    source_locator: PathBuf,
    materialized_path: PathBuf,
    artifact: Rc<ValidatedCrossConeArtifactHandle>,
    closures: ValidatedArtifactClosure,
    agreement: ArtifactFingerprint,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_prebuilt_candidates(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    candidates: Vec<PreparedArtifactCandidate>,
    completed: &[&CompletedNode],

    c_bridge_profile: &CBridgeToolchainProfileV1,
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
        let artifact = plan
            .validate_completed_artifact(
                identity,
                Arc::clone(candidate.snapshot()),
                &artifacts,
                c_bridge_profile,
            )
            .map_err(|source| PrebuiltCompletionError::CandidateArtifact {
                path: candidate.source_locator().to_path_buf(),
                source: Box::new(source),
            })?;
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
        let publication = artifact.publication();
        validated.push(ValidatedCandidate {
            source_locator: candidate.source_locator().to_path_buf(),
            materialized_path: candidate.materialized_path().to_path_buf(),
            agreement: publication.artifact_fingerprint(),
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
        materialized_child_path: PrivateArtifactPath::new(selected.materialized_path),
        warnings: Vec::new(),
    })
}
