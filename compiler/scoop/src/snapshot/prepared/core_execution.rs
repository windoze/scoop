use std::fmt;
use std::sync::Arc;

use scoop_identity::{ArtifactCapabilityProfileId, ConeIdentity};
use scoop_manifest::{ManifestRootError, ManifestRootLocator, load_trusted_core_manifest};
use scoop_protocol::{RequestCorrelationId, ScoopcResponseEnvelopeV1, StructuredDiagnosticV1};
use scoop_slib::{
    ArtifactSnapshot, PrebuiltManifestSummaryError, SlibClosureDecodePurposeV1,
    SlibClosureResourceErrorV1,
};
use scoop_wire::HashError;

use super::model::{
    PreparedArtifactCandidate, PreparedBuildGraph, PreparedGraphNode, TrustedCorePreparation,
    trusted_core_source_key,
};
use super::{PrepareBuildGraphError, capture_manifest_source};
use crate::artifact::{CompletedNode, TrustedCoreCompletionError, complete_trusted_core_candidate};
use crate::{
    CacheReceiptValidationError, ChildProtocolAccountingError, ChildRequestPlanError,
    ChildSuccessArtifactMismatch, ChildTransportError, CompileCacheKeyError,
    ImmutableInputSnapshot, SingleConeCompilerRunner, SnapshotFileError, StagingError,
    TrustedCoreReceiptPublishError, TrustedCoreSlotReceiptBodyV1, TrustedCoreSlotReceiptV1,
    measure_child_request_decode, measure_child_response_decode,
    publish_trusted_core_slot_receipt_v1, validate_child_success_artifact,
};

impl PreparedBuildGraph {
    /// Bootstraps the locked trusted-core slot, then grants completed authority
    /// only after source freshness, dual artifact views, graph shape, child
    /// response, and the newly published receipt all agree.
    pub(crate) fn execute_trusted_core_bootstrap(
        &mut self,
        runner: &mut impl SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
    ) -> Result<CompletedNode, CoreBootstrapExecutionError> {
        let (before_key, receipt_slot) = match self.nodes.get(&ConeIdentity::CORE) {
            Some(PreparedGraphNode::TrustedCore(node)) => {
                if !matches!(node.preparation, TrustedCorePreparation::Bootstrap(_)) {
                    return Err(CoreBootstrapExecutionError::SlotAlreadyReusable);
                }
                (node.source_key, node.receipt_slot.clone())
            }
            _ => unreachable!("resolved graph structurally contains trusted core"),
        };
        let invocation = self
            .child_invocation_plan(ConeIdentity::CORE, request_id, &[])
            .map_err(CoreBootstrapExecutionError::RequestPlan)?;
        let request_usage = measure_child_request_decode(invocation.request())
            .map_err(CoreBootstrapExecutionError::ChildProtocol)?;
        self.meter
            .charge_decode_usage(request_usage)
            .map_err(CoreBootstrapExecutionError::Resource)?;
        self.meter
            .charge_child_request()
            .map_err(CoreBootstrapExecutionError::Resource)?;
        let response = runner
            .invoke(&self.compiler, invocation.request(), invocation.io())
            .map_err(CoreBootstrapExecutionError::ChildTransport)?;
        let response_usage = measure_child_response_decode(&response)
            .map_err(CoreBootstrapExecutionError::ChildProtocol)?;
        self.meter
            .charge_decode_usage(response_usage)
            .map_err(CoreBootstrapExecutionError::Resource)?;
        let success = match response {
            ScoopcResponseEnvelopeV1::Success { result, .. } => result,
            ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                return Err(CoreBootstrapExecutionError::ChildFailure(diagnostics));
            }
        };

        let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
            self.context.sysroot.as_path(),
            self.target_selection,
        );
        let locator = ManifestRootLocator::cone_directory(layout.source_root());
        let loaded = load_trusted_core_manifest(&locator)
            .map_err(CoreBootstrapExecutionError::ReloadSourceManifest)?;
        let after_snapshot = capture_manifest_source(
            loaded,
            &mut self.meter,
            self.context.limits.artifact_decode().semantic_leaf_bytes,
        )
        .map_err(|source| CoreBootstrapExecutionError::SourceSnapshot(Box::new(source)))?;
        let after_key = trusted_core_source_key(
            &after_snapshot,
            &self.compiler,
            &self.context,
            self.target_selection,
        )
        .map_err(|source| CoreBootstrapExecutionError::SourceKey(Box::new(source)))?;
        if after_key != before_key {
            return Err(CoreBootstrapExecutionError::SourceChanged {
                before: before_key,
                after: after_key,
            });
        }

        let candidate = self.capture_bootstrapped_core(invocation.output_path())?;
        let receipt = TrustedCoreSlotReceiptV1::new(
            TrustedCoreSlotReceiptBodyV1::new(
                after_key,
                candidate.summary().artifact_fingerprint(),
                self.target_selection,
                self.compiler.fingerprint(),
                ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
                success.warnings().to_vec(),
            )
            .map_err(CoreBootstrapExecutionError::ReceiptValidation)?,
        )
        .map_err(CoreBootstrapExecutionError::ReceiptHash)?;
        let plan = self.artifact_closure_plan();
        let c_bridge_profile = self.context.target.c_bridge_toolchain().profile().clone();
        let completed = complete_trusted_core_candidate(
            &plan,
            candidate.clone(),
            receipt.clone(),
            after_key,
            self.compiler.fingerprint(),
            self.context.limits.artifact_decode(),
            self.target_selection,
            &c_bridge_profile,
            &mut self.meter,
        )
        .map_err(CoreBootstrapExecutionError::Completion)?;
        validate_child_success_artifact(&success, completed.artifact())
            .map_err(CoreBootstrapExecutionError::ChildResult)?;
        publish_trusted_core_slot_receipt_v1(
            &receipt_slot,
            &receipt,
            self.context.limits.artifact_decode(),
        )
        .map_err(CoreBootstrapExecutionError::PublishReceipt)?;

        match self.nodes.get_mut(&ConeIdentity::CORE) {
            Some(PreparedGraphNode::TrustedCore(node)) => {
                node.snapshot = after_snapshot;
                node.source_key = after_key;
                node.preparation = TrustedCorePreparation::ReuseVerifiedSlot;
                node.existing = Some(candidate);
                node.receipt = Some(receipt);
            }
            _ => unreachable!("resolved graph structurally contains trusted core"),
        }
        Ok(completed)
    }

    fn capture_bootstrapped_core(
        &mut self,
        artifact_slot: &std::path::Path,
    ) -> Result<PreparedArtifactCandidate, CoreBootstrapExecutionError> {
        let limits = self.context.limits.artifact_decode();
        let input = ImmutableInputSnapshot::capture_no_follow(artifact_slot, limits.owned_bytes)
            .map_err(CoreBootstrapExecutionError::OutputSnapshot)?;
        let byte_length = u64::try_from(input.byte_length())
            .map_err(|_| CoreBootstrapExecutionError::OutputLengthOverflow)?;
        self.meter
            .observe_raw_artifact_snapshot(input.digest(), byte_length)
            .map_err(CoreBootstrapExecutionError::Resource)?;
        let snapshot = Arc::new(ArtifactSnapshot::from_shared(input.shared_bytes()));
        let summary = snapshot
            .probe_prebuilt_summary(limits, self.target_selection)
            .map_err(CoreBootstrapExecutionError::OutputSummary)?;
        self.meter
            .observe_artifact_snapshot(&summary, snapshot.digest())
            .map_err(CoreBootstrapExecutionError::Resource)?;
        self.meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::GraphSummary,
                summary.artifact_fingerprint(),
                snapshot.digest(),
                summary.decode_usage(),
            )
            .map_err(CoreBootstrapExecutionError::Resource)?;
        let materialized_path = self
            .staging
            .materialize_verified_core_artifact(snapshot.as_bytes(), snapshot.digest())
            .map_err(CoreBootstrapExecutionError::Staging)?;
        Ok(PreparedArtifactCandidate {
            source_locator: artifact_slot.to_path_buf(),
            materialized_path,
            snapshot,
            summary,
        })
    }
}

#[derive(Debug)]
pub enum CoreBootstrapExecutionError {
    SlotAlreadyReusable,
    RequestPlan(ChildRequestPlanError),
    Resource(SlibClosureResourceErrorV1),
    ChildProtocol(ChildProtocolAccountingError),
    ChildTransport(ChildTransportError),
    ChildFailure(Vec<StructuredDiagnosticV1>),
    ReloadSourceManifest(ManifestRootError),
    SourceSnapshot(Box<PrepareBuildGraphError>),
    SourceKey(Box<CompileCacheKeyError>),
    SourceChanged {
        before: crate::CoreSourceSnapshotKeyV1,
        after: crate::CoreSourceSnapshotKeyV1,
    },
    OutputSnapshot(SnapshotFileError),
    OutputLengthOverflow,
    OutputSummary(PrebuiltManifestSummaryError),
    Staging(StagingError),
    ReceiptValidation(CacheReceiptValidationError),
    ReceiptHash(HashError),
    Completion(TrustedCoreCompletionError),
    ChildResult(ChildSuccessArtifactMismatch),
    PublishReceipt(TrustedCoreReceiptPublishError),
}

impl fmt::Display for CoreBootstrapExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotAlreadyReusable => {
                formatter.write_str("trusted core slot is already reusable")
            }
            Self::RequestPlan(source) => write!(formatter, "cannot plan core child: {source}"),
            Self::Resource(source) => source.fmt(formatter),
            Self::ChildProtocol(source) => {
                write!(
                    formatter,
                    "cannot account for core child protocol frame: {source}"
                )
            }
            Self::ChildTransport(source) => {
                write!(formatter, "trusted core child transport failed: {source}")
            }
            Self::ChildFailure(diagnostics) => write!(
                formatter,
                "trusted core child reported {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::ReloadSourceManifest(source) => {
                write!(
                    formatter,
                    "cannot reload trusted core source manifest: {source}"
                )
            }
            Self::SourceSnapshot(source) => {
                write!(formatter, "cannot resnapshot trusted core source: {source}")
            }
            Self::SourceKey(source) => {
                write!(
                    formatter,
                    "cannot recompute trusted core source key: {source}"
                )
            }
            Self::SourceChanged { before, after } => write!(
                formatter,
                "trusted core source changed during bootstrap: {before} -> {after}"
            ),
            Self::OutputSnapshot(source) => {
                write!(
                    formatter,
                    "cannot snapshot trusted core child output: {source}"
                )
            }
            Self::OutputLengthOverflow => {
                formatter.write_str("trusted core output length does not fit u64")
            }
            Self::OutputSummary(source) => {
                write!(
                    formatter,
                    "cannot probe trusted core child output: {source}"
                )
            }
            Self::Staging(source) => {
                write!(
                    formatter,
                    "cannot materialize validated trusted core: {source}"
                )
            }
            Self::ReceiptValidation(source) => {
                write!(formatter, "cannot construct trusted core receipt: {source}")
            }
            Self::ReceiptHash(source) => {
                write!(
                    formatter,
                    "cannot fingerprint trusted core receipt: {source}"
                )
            }
            Self::Completion(source) => source.fmt(formatter),
            Self::ChildResult(source) => source.fmt(formatter),
            Self::PublishReceipt(source) => {
                write!(formatter, "cannot publish trusted core receipt: {source}")
            }
        }
    }
}

impl std::error::Error for CoreBootstrapExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestPlan(source) => Some(source),
            Self::Resource(source) => Some(source),
            Self::ChildProtocol(source) => Some(source),
            Self::ChildTransport(source) => Some(source),
            Self::ReloadSourceManifest(source) => Some(source),
            Self::SourceSnapshot(source) => Some(source.as_ref()),
            Self::SourceKey(source) => Some(source.as_ref()),
            Self::OutputSnapshot(source) => Some(source),
            Self::OutputSummary(source) => Some(source),
            Self::Staging(source) => Some(source),
            Self::ReceiptValidation(source) => Some(source),
            Self::ReceiptHash(source) => Some(source),
            Self::Completion(source) => Some(source),
            Self::ChildResult(source) => Some(source),
            Self::PublishReceipt(source) => Some(source),
            Self::SlotAlreadyReusable
            | Self::ChildFailure(_)
            | Self::SourceChanged { .. }
            | Self::OutputLengthOverflow => None,
        }
    }
}
