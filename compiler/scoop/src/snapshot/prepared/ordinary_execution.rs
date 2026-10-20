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
    /// Reuse compiled output and complete only missing observations under the key lock.
    pub(crate) fn execute_ordinary_source(
        &mut self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        runner: &mut impl SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
    ) -> Result<(CompletedNode, bool), OrdinarySourceExecutionError> {
        self.execute_source_output(identity, completed, runner, request_id, None)
            .map(|(node, invoked, _)| (node, invoked))
    }

    pub(crate) fn execute_source_output(
        &mut self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        runner: &mut impl SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
        documentation: Option<&std::path::Path>,
    ) -> Result<
        (
            CompletedNode,
            bool,
            Option<Box<scoop_protocol::ScoopcSuccessV1>>,
        ),
        OrdinarySourceExecutionError,
    > {
        use OrdinarySourceExecutionError as Error;
        if !matches!(
            self.nodes.get(&identity),
            Some(PreparedGraphNode::ManifestSource(_) | PreparedGraphNode::SingleFile(_))
        ) {
            return Err(Error::NotOrdinarySource(identity));
        }
        let key = self
            .compile_cache_key(identity, completed)
            .map_err(|source| Error::CacheKey(Box::new(source)))?;
        let store = CompileCacheStoreV1::new(&self.context.cache_root);
        let dumps = store.entry_path(key).join("dumps");
        {
            let lock = store.acquire_shared(key).map_err(Error::CacheStore)?;
            if let CompileCacheLookupV1::Hit(entry) =
                store.lookup(&lock).map_err(Error::CacheStore)?
                && self
                    .missing_dumps(identity, &dumps)
                    .map_err(Error::Observation)?
                    .is_none()
                && documentation.is_none()
            {
                return self
                    .finish_result(
                        identity,
                        *entry,
                        completed,
                        CompletedNodeOrigin::CacheHit,
                        &dumps,
                    )
                    .map(|node| (node, false, None));
            }
        }
        let lock = store.acquire_exclusive(key).map_err(Error::CacheStore)?;
        let cached = store.lookup(&lock).map_err(Error::CacheStore)?;
        if let CompileCacheLookupV1::Hit(entry) = cached {
            let missing = self
                .missing_dumps(identity, &dumps)
                .map_err(Error::Observation)?;
            if let Some(missing) = missing {
                let invocation = self
                    .child_invocation_plan(identity, request_id, completed)
                    .and_then(|plan| plan.observe(missing))
                    .map_err(Error::RequestPlan)?;
                match runner
                    .invoke(&self.compiler, invocation.request(), invocation.io())
                    .map_err(Error::ChildTransport)?
                {
                    ScoopcResponseEnvelopeV1::Observed { result, .. } => self
                        .store_dumps(
                            identity,
                            Some(missing),
                            result.emitted_dump_descriptors(),
                            &dumps,
                        )
                        .map_err(Error::Observation)?,
                    ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                        return Err(Error::ChildFailure(diagnostics));
                    }
                    ScoopcResponseEnvelopeV1::Success { .. } => {
                        return Err(Error::UnexpectedResponse);
                    }
                }
            }
            let documented = if let Some(path) = documentation {
                let invocation = self
                    .child_documentation_plan(identity, request_id, completed, path, false)
                    .map_err(Error::RequestPlan)?;
                match runner
                    .invoke(&self.compiler, invocation.request(), invocation.io())
                    .map_err(Error::ChildTransport)?
                {
                    ScoopcResponseEnvelopeV1::Success { result, .. }
                        if result.documentation().is_some() =>
                    {
                        Some(result)
                    }
                    ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                        return Err(Error::ChildFailure(diagnostics));
                    }
                    _ => return Err(Error::UnexpectedResponse),
                }
            } else {
                None
            };
            return self
                .finish_result(
                    identity,
                    *entry,
                    completed,
                    CompletedNodeOrigin::CacheHit,
                    &dumps,
                )
                .map(|node| (node, missing.is_some() || documented.is_some(), documented));
        }
        let invocation = match documentation {
            Some(path) => {
                self.child_documentation_plan(identity, request_id, completed, path, true)
            }
            None => self.child_invocation_plan(identity, request_id, completed),
        }
        .map_err(Error::RequestPlan)?;
        let success = match runner
            .invoke(&self.compiler, invocation.request(), invocation.io())
            .map_err(Error::ChildTransport)?
        {
            ScoopcResponseEnvelopeV1::Success { result, .. } => result,
            ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                return Err(Error::ChildFailure(diagnostics));
            }
            ScoopcResponseEnvelopeV1::Observed { .. } => return Err(Error::UnexpectedResponse),
        };
        let result = (|| {
            self.store_dumps(identity, None, success.emitted_dump_descriptors(), &dumps)
                .map_err(Error::Observation)?;
            let entry = store
                .publish(
                    &lock,
                    invocation.output_path(),
                    CompileBuildRecordV1 {
                        summary: success.summary().ok_or(Error::UnexpectedResponse)?.clone(),
                        warnings: success.warnings().to_vec(),
                    },
                )
                .map_err(Error::CacheStore)?;
            self.finish_result(
                identity,
                entry,
                completed,
                CompletedNodeOrigin::Compiled,
                &dumps,
            )
        })();
        result
            .map(|node| (node, true, documentation.map(|_| success.clone())))
            .map_err(|source| {
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

    fn finish_result(
        &self,
        identity: ConeIdentity,
        entry: CompileCacheEntryV1,
        completed: &[&CompletedNode],
        origin: CompletedNodeOrigin,
        dumps: &std::path::Path,
    ) -> Result<CompletedNode, OrdinarySourceExecutionError> {
        self.publish_cached_dumps(identity, dumps)
            .map_err(OrdinarySourceExecutionError::Observation)?;
        self.complete_result(identity, entry, completed, origin)
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
