use std::fmt;
use std::sync::Arc;

use scoop_identity::ConeIdentity;
use scoop_protocol::{RequestCorrelationId, ScoopcResponseEnvelopeV1, StructuredDiagnosticV1};
use scoop_slib::{ArtifactSnapshot, ConeRecord, ConeRecordError};
use scoop_wire::HashError;

use super::model::{PreparedBuildGraph, PreparedGraphNode};
use crate::artifact::{CompiledCompletionError, CompletedNode, complete_compiled_candidate};
use crate::{
    CacheCompletionError, CacheReceiptBodyV1, CacheReceiptV1, CacheReceiptValidationError,
    ChildRequestPlanError, ChildSuccessArtifactMismatch, ChildTransportError, CompileCacheKeyError,
    CompileCacheLookupV1, CompileCacheStoreError, CompileCacheStoreV1, ImmutableInputSnapshot,
    SingleConeCompilerRunner, SnapshotFileError, validate_child_success_artifact,
};

impl PreparedBuildGraph {
    /// Resolves one ordinary source node through the content-addressed cache
    /// or exactly one paired compiler child while the exclusive key lock is
    /// held across the miss, validation, and atomic publication.
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
        {
            let lock = store
                .acquire_shared(key)
                .map_err(OrdinarySourceExecutionError::CacheStore)?;
            if let CompileCacheLookupV1::Hit(entry) = store
                .lookup(&lock, self.context.limits.artifact_decode())
                .map_err(OrdinarySourceExecutionError::CacheStore)?
            {
                return self
                    .complete_cache_hit(identity, *entry, completed)
                    .map_err(OrdinarySourceExecutionError::CacheCompletion);
            }
        }

        let lock = store
            .acquire_exclusive(key)
            .map_err(OrdinarySourceExecutionError::CacheStore)?;
        if let CompileCacheLookupV1::Hit(entry) = store
            .lookup(&lock, self.context.limits.artifact_decode())
            .map_err(OrdinarySourceExecutionError::CacheStore)?
        {
            return self
                .complete_cache_hit(identity, *entry, completed)
                .map_err(OrdinarySourceExecutionError::CacheCompletion);
        }

        let invocation = self
            .child_invocation_plan(identity, request_id, completed)
            .map_err(OrdinarySourceExecutionError::RequestPlan)?;
        self.staging
            .require_empty_output(invocation.output_path())
            .map_err(OrdinarySourceExecutionError::OutputLayout)?;

        let response = runner
            .invoke(&self.compiler, invocation.request(), invocation.io())
            .map_err(OrdinarySourceExecutionError::ChildTransport)?;

        let success = match response {
            ScoopcResponseEnvelopeV1::Success { result, .. } => result,
            ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                return Err(OrdinarySourceExecutionError::ChildFailure(diagnostics));
            }
        };
        self.staging
            .validate_completed_output(invocation.output_path())
            .map_err(OrdinarySourceExecutionError::OutputLayout)?;
        let output = ImmutableInputSnapshot::capture_no_follow(
            invocation.output_path(),
            self.context.limits.artifact_decode().owned_bytes,
        )
        .map_err(OrdinarySourceExecutionError::OutputSnapshot)?;
        let snapshot = Arc::new(ArtifactSnapshot::from_shared(output.shared_bytes()));
        let plan = self.artifact_closure_plan();
        let c_bridge_profile = self.context.target.c_bridge_toolchain().profile().clone();
        let mut completed_node = complete_compiled_candidate(
            &plan,
            identity,
            snapshot,
            invocation.output_path().to_path_buf(),
            completed,
            success.warnings().to_vec(),
            self.context.limits.artifact_decode(),
            &c_bridge_profile,
        )
        .map_err(OrdinarySourceExecutionError::Completion)?;
        validate_child_success_artifact(&success, completed_node.artifact())
            .map_err(OrdinarySourceExecutionError::ChildResult)?;

        let publication = completed_node.artifact().publication();
        let cone = ConeRecord::new(
            publication.coordinate().clone(),
            publication.kind(),
            publication.source_form(),
        )
        .map_err(OrdinarySourceExecutionError::ConeRecord)?;
        let receipt = CacheReceiptV1::new(
            CacheReceiptBodyV1::new(
                key,
                publication.artifact_fingerprint(),
                cone,
                publication.target_selection(),
                publication.direct_dependencies().to_vec(),
                self.compiler.fingerprint(),
                publication.profile().clone(),
                success.warnings().to_vec(),
            )
            .map_err(OrdinarySourceExecutionError::ReceiptValidation)?,
        )
        .map_err(OrdinarySourceExecutionError::ReceiptHash)?;
        completed_node.replace_warnings(receipt.body().structured_warnings().to_vec());
        store
            .publish(
                &lock,
                &output,
                &receipt,
                self.context.limits.artifact_decode(),
            )
            .map_err(OrdinarySourceExecutionError::CacheStore)?;

        Ok(completed_node)
    }
}

#[derive(Debug)]
pub enum OrdinarySourceExecutionError {
    NotOrdinarySource(ConeIdentity),
    CacheKey(Box<CompileCacheKeyError>),
    CacheStore(CompileCacheStoreError),
    CacheCompletion(CacheCompletionError),
    RequestPlan(ChildRequestPlanError),

    ChildTransport(ChildTransportError),
    ChildFailure(Vec<StructuredDiagnosticV1>),
    OutputLayout(crate::StagingError),
    OutputSnapshot(SnapshotFileError),
    Completion(CompiledCompletionError),
    ChildResult(ChildSuccessArtifactMismatch),
    ConeRecord(ConeRecordError),
    ReceiptValidation(CacheReceiptValidationError),
    ReceiptHash(HashError),
}

impl fmt::Display for OrdinarySourceExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOrdinarySource(identity) => {
                write!(formatter, "Cone {identity} is not an ordinary source node")
            }
            Self::CacheKey(source) => write!(formatter, "cannot derive cache key: {source}"),
            Self::CacheStore(source) => source.fmt(formatter),
            Self::CacheCompletion(source) => source.fmt(formatter),
            Self::RequestPlan(source) => write!(formatter, "cannot plan compiler child: {source}"),

            Self::ChildTransport(source) => write!(formatter, "child transport failed: {source}"),
            Self::ChildFailure(diagnostics) => write!(
                formatter,
                "compiler child reported {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::OutputLayout(source) => {
                write!(formatter, "invalid private child output layout: {source}")
            }
            Self::OutputSnapshot(source) => {
                write!(formatter, "cannot snapshot compiler child output: {source}")
            }
            Self::Completion(source) => source.fmt(formatter),
            Self::ChildResult(source) => source.fmt(formatter),
            Self::ConeRecord(source) => write!(formatter, "invalid compiled Cone: {source}"),
            Self::ReceiptValidation(source) => {
                write!(formatter, "cannot construct cache receipt: {source}")
            }
            Self::ReceiptHash(source) => {
                write!(formatter, "cannot fingerprint cache receipt: {source}")
            }
        }
    }
}

impl std::error::Error for OrdinarySourceExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CacheKey(source) => Some(source.as_ref()),
            Self::CacheStore(source) => Some(source),
            Self::CacheCompletion(source) => Some(source),
            Self::RequestPlan(source) => Some(source),

            Self::ChildTransport(source) => Some(source),
            Self::OutputLayout(source) => Some(source),
            Self::OutputSnapshot(source) => Some(source),
            Self::Completion(source) => Some(source),
            Self::ChildResult(source) => Some(source),
            Self::ConeRecord(source) => Some(source),
            Self::ReceiptValidation(source) => Some(source),
            Self::ReceiptHash(source) => Some(source),
            Self::NotOrdinarySource(_) | Self::ChildFailure(_) => None,
        }
    }
}
