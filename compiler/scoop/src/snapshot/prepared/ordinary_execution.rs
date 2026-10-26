use super::model::{PreparedBuildGraph, PreparedGraphNode, child_request::ChildInvocationPlanV1};
use crate::{
    ArtifactClosureValidationError, BuildArtifact, ChildRequestPlanError, ChildTransportError,
    CompileBuildRecordV1, CompileCacheEntryV1, CompileCacheKeyError, CompileCacheKeyLockV1,
    CompileCacheLookupV1, CompileCacheStoreError, CompileCacheStoreV1, CompletedNode,
    CompletedNodeOrigin, ConeCompileCacheKeyV1,
};
use scoop_identity::ConeIdentity;
use scoop_protocol::{
    RequestCorrelationId, ScoopcResponseEnvelopeV1, StageDumpSet, StructuredDiagnosticV1,
};
use std::collections::BTreeMap;
use std::fmt;

pub(crate) enum PreparedSourceExecution {
    Complete(Box<CompletedNode>),
    Pending(Box<PendingSourceExecution>),
    WaitingForLock,
}

pub(crate) struct PendingSourceExecution {
    identity: ConeIdentity,
    pub(crate) invocation: ChildInvocationPlanV1,
    lock: CompileCacheKeyLockV1,
    operation: PendingOperation,
}

enum PendingOperation {
    Build,
    Observe {
        entry: Box<CompileCacheEntryV1>,
        missing: StageDumpSet,
    },
}

impl PreparedBuildGraph {
    /// Check reuse before preparing child work, retaining the key lock until completion.
    pub(crate) fn prepare_ordinary_source(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        key: ConeCompileCacheKeyV1,
        request_id: RequestCorrelationId,
        wait_for_lock: bool,
    ) -> Result<PreparedSourceExecution, OrdinarySourceExecutionError> {
        self.prepare_source_output(identity, completed, key, request_id, wait_for_lock, None)
    }

    pub(super) fn prepare_source_output(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        key: ConeCompileCacheKeyV1,
        request_id: RequestCorrelationId,
        wait_for_lock: bool,
        documentation: Option<&std::path::Path>,
    ) -> Result<PreparedSourceExecution, OrdinarySourceExecutionError> {
        use OrdinarySourceExecutionError as Error;
        if !matches!(
            self.nodes.get(&identity),
            Some(PreparedGraphNode::ManifestSource(_) | PreparedGraphNode::SingleFile(_))
        ) {
            return Err(Error::NotOrdinarySource(identity));
        }
        let store = CompileCacheStoreV1::new(&self.context.cache_root);
        {
            let Some(lock) = store
                .acquire_available(key, crate::CompileCacheLockModeV1::Shared, wait_for_lock)
                .map_err(Error::CacheStore)?
            else {
                return Ok(PreparedSourceExecution::WaitingForLock);
            };
            if let CompileCacheLookupV1::Hit(entry) = self.lookup_current(&store, &lock)?
                && self
                    .missing_dumps(identity, &entry.dumps_path())
                    .map_err(Error::Observation)?
                    .is_none()
            {
                return self
                    .finish_result(identity, *entry, completed, CompletedNodeOrigin::CacheHit)
                    .map(Box::new)
                    .map(PreparedSourceExecution::Complete);
            }
        }
        let Some(lock) = store
            .acquire_available(key, crate::CompileCacheLockModeV1::Exclusive, wait_for_lock)
            .map_err(Error::CacheStore)?
        else {
            return Ok(PreparedSourceExecution::WaitingForLock);
        };
        let cached = self.lookup_current(&store, &lock)?;
        let operation = if let CompileCacheLookupV1::Hit(entry) = cached {
            let missing = self
                .missing_dumps(identity, &entry.dumps_path())
                .map_err(Error::Observation)?;
            let Some(missing) = missing else {
                return self
                    .finish_result(identity, *entry, completed, CompletedNodeOrigin::CacheHit)
                    .map(Box::new)
                    .map(PreparedSourceExecution::Complete);
            };
            PendingOperation::Observe { entry, missing }
        } else {
            PendingOperation::Build
        };
        let invocation = match &operation {
            PendingOperation::Build => match documentation {
                Some(path) => {
                    self.child_documentation_plan(identity, request_id, completed, path, true)
                }
                None => self.child_invocation_plan(identity, request_id, completed),
            },
            PendingOperation::Observe { missing, .. } => self
                .child_invocation_plan(identity, request_id, completed)
                .and_then(|plan| plan.observe(*missing)),
        }
        .map_err(Error::RequestPlan)?;
        Ok(PreparedSourceExecution::Pending(Box::new(
            PendingSourceExecution {
                identity,
                invocation,
                lock,
                operation,
            },
        )))
    }

    pub(crate) fn finish_ordinary_source(
        &self,
        pending: PendingSourceExecution,
        completed: &[&CompletedNode],
        response: Result<ScoopcResponseEnvelopeV1, ChildTransportError>,
    ) -> Result<CompletedNode, OrdinarySourceExecutionError> {
        use OrdinarySourceExecutionError as Error;
        let PendingSourceExecution {
            identity,
            invocation,
            lock,
            operation,
        } = pending;
        let store = CompileCacheStoreV1::new(&self.context.cache_root);
        match (operation, response.map_err(Error::ChildTransport)?) {
            (_, ScoopcResponseEnvelopeV1::Failure { diagnostics, .. }) => {
                Err(Error::ChildFailure(diagnostics))
            }
            (
                PendingOperation::Observe { entry, missing },
                ScoopcResponseEnvelopeV1::Observed { result, .. },
            ) => {
                self.store_dumps(
                    identity,
                    Some(missing),
                    result.emitted_dump_descriptors(),
                    &entry.dumps_path(),
                )
                .map_err(Error::Observation)?;
                self.finish_result(identity, *entry, completed, CompletedNodeOrigin::CacheHit)
            }
            (
                PendingOperation::Build,
                ScoopcResponseEnvelopeV1::Success {
                    result: success, ..
                },
            ) => {
                let result = (|| {
                    let summary = success.summary().ok_or(Error::UnexpectedResponse)?;
                    let dumps = store.result_directory(lock.key(), summary).join("dumps");
                    self.store_dumps(identity, None, success.emitted_dump_descriptors(), &dumps)
                        .map_err(Error::Observation)?;
                    let entry = store
                        .publish(
                            &lock,
                            invocation.output_path(),
                            CompileBuildRecordV1 {
                                summary: summary.clone(),
                                warnings: success.warnings().to_vec(),
                                used_generators: success.used_generators().to_vec(),
                            },
                        )
                        .map_err(Error::CacheStore)?;
                    self.finish_result(identity, entry, completed, CompletedNodeOrigin::Compiled)
                })();
                result.map_err(|source| {
                    if success.warnings().is_empty() {
                        source
                    } else {
                        Error::ProducedOutput {
                            source: Box::new(source),
                            warnings: success.warnings().to_vec(),
                        }
                    }
                })
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    #[cfg(test)]
    pub(crate) fn execute_ordinary_source(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        runner: &impl crate::SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
    ) -> Result<(CompletedNode, bool), OrdinarySourceExecutionError> {
        let key = self
            .compile_cache_key(identity, completed)
            .map_err(|source| OrdinarySourceExecutionError::CacheKey(Box::new(source)))?;
        match self.prepare_ordinary_source(identity, completed, key, request_id, true)? {
            PreparedSourceExecution::WaitingForLock => {
                unreachable!("blocking acquisition completes")
            }
            PreparedSourceExecution::Complete(node) => Ok((*node, false)),
            PreparedSourceExecution::Pending(pending) => {
                let response = runner.invoke(
                    &self.compiler,
                    pending.invocation.request(),
                    pending.invocation.io(),
                );
                self.finish_ordinary_source(*pending, completed, response)
                    .map(|node| (node, true))
            }
        }
    }

    fn finish_result(
        &self,
        identity: ConeIdentity,
        entry: CompileCacheEntryV1,
        completed: &[&CompletedNode],
        origin: CompletedNodeOrigin,
    ) -> Result<CompletedNode, OrdinarySourceExecutionError> {
        self.publish_cached_dumps(identity, &entry.dumps_path())
            .map_err(OrdinarySourceExecutionError::Observation)?;
        self.complete_result(identity, entry, completed, origin)
            .map_err(OrdinarySourceExecutionError::Completion)
    }

    fn lookup_current(
        &self,
        store: &CompileCacheStoreV1,
        lock: &CompileCacheKeyLockV1,
    ) -> Result<CompileCacheLookupV1, OrdinarySourceExecutionError> {
        let cached = store
            .lookup(lock)
            .map_err(OrdinarySourceExecutionError::CacheStore)?;
        if let CompileCacheLookupV1::Hit(entry) = &cached
            && !self
                .context
                .generators
                .inputs_current(
                    &entry.record.used_generators,
                    self.compiler.executable_path(),
                )
                .map_err(OrdinarySourceExecutionError::Generators)?
        {
            return Ok(CompileCacheLookupV1::Miss);
        }
        Ok(cached)
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
        )
        .with_used_generators(entry.record.used_generators))
    }
}

#[derive(Debug)]
pub enum OrdinarySourceExecutionError {
    Generators(scoop_toolchain::ToolchainError),
    ProducedOutput {
        source: Box<OrdinarySourceExecutionError>,
        warnings: Vec<StructuredDiagnosticV1>,
    },
    Observation(std::io::Error),
    UnexpectedResponse,
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
            Self::Generators(source) => source.fmt(formatter),
            Self::ProducedOutput { source, .. } => source.fmt(formatter),
            Self::UnexpectedResponse => {
                formatter.write_str("compiler response does not match the requested operation")
            }
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
            Self::Generators(source) => Some(source),
            Self::ProducedOutput { source, .. } => Some(source.as_ref()),
            Self::Observation(source) => Some(source),
            Self::CacheKey(source) => Some(source.as_ref()),
            Self::CacheStore(source) => Some(source),
            Self::RequestPlan(source) => Some(source),

            Self::ChildTransport(source) => Some(source),
            Self::Completion(source) => Some(source),
            Self::UnexpectedResponse | Self::NotOrdinarySource(_) | Self::ChildFailure(_) => None,
        }
    }
}
