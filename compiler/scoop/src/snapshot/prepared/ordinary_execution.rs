use super::model::{PreparedBuildGraph, PreparedGraphNode};
use crate::{
    ArtifactClosureValidationError, BuildArtifact, ChildRequestPlanError, ChildTransportError,
    CompileBuildRecordV1, CompileCacheEntryV1, CompileCacheKeyError, CompileCacheLookupV1,
    CompileCacheStoreError, CompileCacheStoreV1, CompletedNode, CompletedNodeOrigin,
    SingleConeCompilerRunner,
};
use scoop_identity::ConeIdentity;
use scoop_protocol::{RequestCorrelationId, ScoopcResponseEnvelopeV1, StructuredDiagnosticV1};
use std::collections::BTreeMap;
use std::fmt;

impl PreparedBuildGraph {
    /// Observed sources compile independently; ordinary misses keep their key
    /// lock through compilation. Both publish under the same exclusive key lock.
    pub(crate) fn execute_ordinary_source(
        &mut self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        runner: &mut impl SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
    ) -> Result<CompletedNode, OrdinarySourceExecutionError> {
        if !matches!(
            self.nodes.get(&identity),
            Some(PreparedGraphNode::ManifestSource(_) | PreparedGraphNode::SingleFile(_))
        ) {
            return Err(OrdinarySourceExecutionError::NotOrdinarySource(identity));
        }
        let key = self
            .compile_cache_key(identity, completed)
            .map_err(|source| OrdinarySourceExecutionError::CacheKey(Box::new(source)))?;
        let store = CompileCacheStoreV1::new(&self.context.cache_root);
        let lock = if self.is_observed(identity) {
            None
        } else {
            {
                let lock = store
                    .acquire_shared(key)
                    .map_err(OrdinarySourceExecutionError::CacheStore)?;
                if let CompileCacheLookupV1::Hit(entry) = store
                    .lookup(&lock)
                    .map_err(OrdinarySourceExecutionError::CacheStore)?
                {
                    return self
                        .complete_result(identity, *entry, completed, CompletedNodeOrigin::CacheHit)
                        .map_err(OrdinarySourceExecutionError::Completion);
                }
            }

            let lock = store
                .acquire_exclusive(key)
                .map_err(OrdinarySourceExecutionError::CacheStore)?;
            if let CompileCacheLookupV1::Hit(entry) = store
                .lookup(&lock)
                .map_err(OrdinarySourceExecutionError::CacheStore)?
            {
                return self
                    .complete_result(identity, *entry, completed, CompletedNodeOrigin::CacheHit)
                    .map_err(OrdinarySourceExecutionError::Completion);
            }
            Some(lock)
        };

        let invocation = self
            .child_invocation_plan(identity, request_id, completed)
            .map_err(OrdinarySourceExecutionError::RequestPlan)?;

        let response = runner
            .invoke(&self.compiler, invocation.request(), invocation.io())
            .map_err(OrdinarySourceExecutionError::ChildTransport)?;

        let success = match response {
            ScoopcResponseEnvelopeV1::Success { result, .. } => result,
            ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                return Err(OrdinarySourceExecutionError::ChildFailure(diagnostics));
            }
        };
        self.finish_source_output(identity, completed, &invocation, key, lock, &success)
            .map_err(|source| {
                if success.warnings().is_empty() {
                    source
                } else {
                    OrdinarySourceExecutionError::ProducedOutput {
                        source: Box::new(source),
                        warnings: success.warnings().to_vec(),
                    }
                }
            })
    }

    fn finish_source_output(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        invocation: &super::model::child_request::ChildInvocationPlanV1,
        key: crate::ConeCompileCacheKeyV1,
        held_lock: Option<crate::CompileCacheKeyLockV1>,
        success: &scoop_protocol::ScoopcSuccessV1,
    ) -> Result<CompletedNode, OrdinarySourceExecutionError> {
        let store = CompileCacheStoreV1::new(&self.context.cache_root);
        self.publish_dumps(identity, success.emitted_dump_descriptors())
            .map_err(OrdinarySourceExecutionError::Observation)?;
        let lock = match held_lock {
            Some(lock) => lock,
            None => {
                let lock = store
                    .acquire_exclusive(key)
                    .map_err(OrdinarySourceExecutionError::CacheStore)?;
                if let CompileCacheLookupV1::Hit(entry) = store
                    .lookup(&lock)
                    .map_err(OrdinarySourceExecutionError::CacheStore)?
                {
                    return self
                        .complete_result(identity, *entry, completed, CompletedNodeOrigin::Compiled)
                        .map_err(OrdinarySourceExecutionError::Completion);
                }
                lock
            }
        };
        let entry = store
            .publish(
                &lock,
                invocation.output_path(),
                CompileBuildRecordV1 {
                    summary: success.summary().clone(),
                    warnings: success.warnings().to_vec(),
                },
            )
            .map_err(OrdinarySourceExecutionError::CacheStore)?;
        self.complete_result(identity, entry, completed, CompletedNodeOrigin::Compiled)
            .map_err(OrdinarySourceExecutionError::Completion)
    }

    fn complete_result(
        &self,
        identity: ConeIdentity,
        entry: CompileCacheEntryV1,
        completed: &[&CompletedNode],
        origin: CompletedNodeOrigin,
    ) -> Result<CompletedNode, ArtifactClosureValidationError> {
        let artifact = BuildArtifact::from_result(entry.artifact.clone(), entry.record.summary);
        let mut artifacts: BTreeMap<_, _> = completed
            .iter()
            .map(|node| (node.cone(), node.shared_artifact()))
            .collect();
        artifacts.insert(identity, std::rc::Rc::clone(&artifact));
        let closure = self.artifact_closure_plan().collect(identity, &artifacts)?;
        Ok(CompletedNode::from_result(
            identity,
            origin,
            artifact,
            closure,
            entry.artifact,
            entry.record.warnings,
        ))
    }
}

#[derive(Debug)]
pub enum OrdinarySourceExecutionError {
    ProducedOutput {
        source: Box<OrdinarySourceExecutionError>,
        warnings: Vec<StructuredDiagnosticV1>,
    },
    Observation(std::io::Error),
    NotOrdinarySource(ConeIdentity),
    CacheKey(Box<CompileCacheKeyError>),
    CacheStore(CompileCacheStoreError),
    RequestPlan(ChildRequestPlanError),

    ChildTransport(ChildTransportError),
    ChildFailure(Vec<StructuredDiagnosticV1>),
    Completion(ArtifactClosureValidationError),
}

impl fmt::Display for OrdinarySourceExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProducedOutput { source, .. } => source.fmt(formatter),
            Self::Observation(source) => {
                write!(formatter, "cannot observe compiler stages: {source}")
            }
            Self::NotOrdinarySource(identity) => {
                write!(formatter, "Cone {identity} is not an ordinary source node")
            }
            Self::CacheKey(source) => write!(formatter, "cannot derive cache key: {source}"),
            Self::CacheStore(source) => source.fmt(formatter),
            Self::RequestPlan(source) => write!(formatter, "cannot plan compiler child: {source}"),

            Self::ChildTransport(source) => write!(formatter, "child transport failed: {source}"),
            Self::ChildFailure(diagnostics) => write!(
                formatter,
                "compiler child reported {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Completion(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinarySourceExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProducedOutput { source, .. } => Some(source.as_ref()),
            Self::Observation(source) => Some(source),
            Self::CacheKey(source) => Some(source.as_ref()),
            Self::CacheStore(source) => Some(source),
            Self::RequestPlan(source) => Some(source),

            Self::ChildTransport(source) => Some(source),
            Self::Completion(source) => Some(source),
            Self::NotOrdinarySource(_) | Self::ChildFailure(_) => None,
        }
    }
}
