use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_slib::{
    ConeKind, ConeRecord, ConeRecordError, ConeSourceForm, FingerprintAvailability,
    IdentityAbiDescriptor,
};
use scoop_wire::HashError;

use super::{PreparedBuildGraph, PreparedGraphNode};
use crate::artifact::{ArtifactClosureValidationError, CompletedNode};
use crate::cache::{
    CompileDependencyInputV1, ConeCompileCacheInputV1, ConeCompileCacheKeyV1,
    CurrentConeSemanticProjectionV1, OptionalCoreCodeFingerprintV1, SourceCacheInputV1,
    strong_profile_id,
};
use crate::discovery::compare_coordinates;

impl PreparedBuildGraph {
    /// Constructs the normalized semantic input for one source node after all
    /// of its direct dependencies have completed.
    pub fn compile_cache_input(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
    ) -> Result<ConeCompileCacheInputV1, CompileCacheKeyError> {
        let (cone, current_semantic, mut sources) = self.current_cache_inputs(identity)?;
        sources.sort_by(|left, right| left.source().cmp(right.source()));
        if let Some(repeated) = sources
            .windows(2)
            .find(|pair| pair[0].source() == pair[1].source())
        {
            return Err(CompileCacheKeyError::DuplicateSource(
                repeated[0].source().clone(),
            ));
        }

        let completed = completed_map(identity, completed)?;
        let direct: BTreeSet<_> = self
            .edges
            .values()
            .filter(|edge| edge.dependent() == identity)
            .map(|edge| edge.dependency())
            .collect();
        let plan = self.artifact_closure_plan();
        let mut dependencies = Vec::with_capacity(direct.len());
        for dependency in direct {
            let completed =
                completed
                    .get(&dependency)
                    .ok_or(CompileCacheKeyError::MissingDependency {
                        dependent: identity,
                        dependency,
                    })?;
            plan.validate_artifact_shape(dependency, completed.artifact())
                .map_err(|source| CompileCacheKeyError::DependencyPlan {
                    dependent: identity,
                    dependency,
                    source: Box::new(source),
                })?;
            let summary = completed.artifact().summary();
            let semantic = summary.semantic_fingerprints();
            let single_file_core_code =
                if identity == ConeIdentity::SINGLE_FILE && dependency == ConeIdentity::CORE {
                    match semantic.code() {
                        FingerprintAvailability::Available(code) => {
                            OptionalCoreCodeFingerprintV1::Some(code)
                        }
                        FingerprintAvailability::Unavailable => {
                            return Err(CompileCacheKeyError::MissingSingleFileCoreCode);
                        }
                    }
                } else {
                    OptionalCoreCodeFingerprintV1::None
                };
            dependencies.push(CompileDependencyInputV1::new(
                summary.cone().clone(),
                semantic.hir(),
                semantic.mir(),
                semantic.lir(),
                single_file_core_code,
            ));
        }
        dependencies.sort_by(|left, right| {
            compare_coordinates(left.cone().coordinate(), right.cone().coordinate())
        });

        let compatibility = IdentityAbiDescriptor::current().map_err(CompileCacheKeyError::Hash)?;
        let lir_target = self
            .target_selection
            .target()
            .fingerprint()
            .map_err(CompileCacheKeyError::Hash)?;
        let backend = self
            .target_selection
            .backend()
            .fingerprint()
            .map_err(CompileCacheKeyError::Hash)?;
        let c_bridge_toolchain = self
            .context
            .target
            .c_bridge_toolchain()
            .profile()
            .fingerprint();
        Ok(ConeCompileCacheInputV1::new(
            cone,
            current_semantic,
            sources,
            dependencies,
            self.compiler.fingerprint(),
            compatibility,
            strong_profile_id(),
            self.compiler.protocol(),
            lir_target,
            backend,
            c_bridge_toolchain,
        )
        .with_optimization(self.context.optimization))
    }

    pub fn compile_cache_key(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
    ) -> Result<ConeCompileCacheKeyV1, CompileCacheKeyError> {
        self.compile_cache_input(identity, completed)?
            .key()
            .map_err(CompileCacheKeyError::Hash)
    }

    fn current_cache_inputs(
        &self,
        identity: ConeIdentity,
    ) -> Result<
        (
            ConeRecord,
            CurrentConeSemanticProjectionV1,
            Vec<SourceCacheInputV1>,
        ),
        CompileCacheKeyError,
    > {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::ManifestSource(node)) => {
                manifest_cache_inputs(&node.snapshot, node.snapshot.requested_kind)
            }
            Some(PreparedGraphNode::SingleFile(node)) => {
                let cone = ConeRecord::new(
                    ConeCoordinate::reserved_single_file(),
                    ConeKind::Executable,
                    ConeSourceForm::SingleFile,
                )
                .map_err(CompileCacheKeyError::ConeRecord)?;
                Ok((
                    cone,
                    CurrentConeSemanticProjectionV1::SingleFile,
                    vec![SourceCacheInputV1::new(
                        node.snapshot.source.identity().clone(),
                        node.snapshot.source.content_digest(),
                    )],
                ))
            }
            Some(PreparedGraphNode::PrebuiltArtifact(_)) => {
                Err(CompileCacheKeyError::PrebuiltNode(identity))
            }
            None => Err(CompileCacheKeyError::UnknownNode(identity)),
        }
    }
}

fn manifest_cache_inputs(
    snapshot: &super::ManifestSourceSnapshot,
    requested_kind: RequestedConeKind,
) -> Result<
    (
        ConeRecord,
        CurrentConeSemanticProjectionV1,
        Vec<SourceCacheInputV1>,
    ),
    CompileCacheKeyError,
> {
    let cone = ConeRecord::new(
        snapshot.coordinate.clone(),
        match requested_kind {
            RequestedConeKind::Library => ConeKind::Library,
            RequestedConeKind::Executable => ConeKind::Executable,
        },
        ConeSourceForm::Manifest,
    )
    .map_err(CompileCacheKeyError::ConeRecord)?;
    let mut dependencies: Vec<_> = snapshot
        .manifest_semantic
        .dependency_iter()
        .map(|(_, coordinate)| coordinate.clone())
        .collect();
    dependencies.sort_by(compare_coordinates);
    Ok((
        cone,
        CurrentConeSemanticProjectionV1::Manifest {
            coordinate: snapshot.coordinate.clone(),
            requested_kind,
            dependencies,
            sources: snapshot.manifest_semantic.sources().clone(),
            native: Box::new(snapshot.native.clone()),
        },
        source_inputs(snapshot.sources()),
    ))
}

fn source_inputs<'a>(
    sources: impl Iterator<Item = &'a super::SourceSnapshot>,
) -> Vec<SourceCacheInputV1> {
    sources
        .map(|source| SourceCacheInputV1::new(source.identity().clone(), source.content_digest()))
        .collect()
}

fn completed_map<'a>(
    current: ConeIdentity,
    completed: &'a [&'a CompletedNode],
) -> Result<BTreeMap<ConeIdentity, &'a CompletedNode>, CompileCacheKeyError> {
    let mut by_identity = BTreeMap::new();
    for node in completed {
        if node.cone() == current {
            return Err(CompileCacheKeyError::CurrentNodeAlreadyCompleted(current));
        }
        if by_identity.insert(node.cone(), *node).is_some() {
            return Err(CompileCacheKeyError::DuplicateCompletedNode(node.cone()));
        }
    }
    Ok(by_identity)
}

#[derive(Debug)]
pub enum CompileCacheKeyError {
    UnknownNode(ConeIdentity),
    PrebuiltNode(ConeIdentity),
    DuplicateSource(scoop_identity::SourceIdentity),
    DuplicateCompletedNode(ConeIdentity),
    CurrentNodeAlreadyCompleted(ConeIdentity),
    MissingDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    DependencyPlan {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        source: Box<ArtifactClosureValidationError>,
    },
    MissingSingleFileCoreCode,
    ConeRecord(ConeRecordError),
    Hash(HashError),
}

impl fmt::Display for CompileCacheKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(identity) => write!(formatter, "unknown build node {identity}"),
            Self::PrebuiltNode(identity) => write!(
                formatter,
                "prebuilt node {identity} does not have a source compile cache key"
            ),
            Self::DuplicateSource(source) => write!(
                formatter,
                "compile cache input repeats source {}",
                source.logical_path()
            ),
            Self::DuplicateCompletedNode(identity) => {
                write!(
                    formatter,
                    "compile cache input repeats completed Cone {identity}"
                )
            }
            Self::CurrentNodeAlreadyCompleted(identity) => write!(
                formatter,
                "current Cone {identity} was supplied as its own completed dependency"
            ),
            Self::MissingDependency {
                dependent,
                dependency,
            } => write!(
                formatter,
                "cannot key Cone {dependent} before dependency {dependency} completes"
            ),
            Self::DependencyPlan {
                dependent,
                dependency,
                source,
            } => write!(
                formatter,
                "completed dependency {dependency} does not match cache plan for {dependent}: {source}"
            ),
            Self::MissingSingleFileCoreCode => formatter
                .write_str("single-file cache key requires the trusted core Code fingerprint"),
            Self::ConeRecord(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileCacheKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DependencyPlan { source, .. } => Some(source),
            Self::ConeRecord(error) => Some(error),
            Self::Hash(error) => Some(error),
            _ => None,
        }
    }
}
