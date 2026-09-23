use std::borrow::Cow;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{
    PrebuiltManifestSummaryV1, SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1,
    probe_prebuilt_manifest_summary,
};
use scoop_wire::{DecodeLimits, sha256};

use super::*;

impl LoadedExplicitDependencyArtifact {
    pub(super) fn summary(
        &self,
        limits: DecodeLimits,
        target: ValidatedLirTargetSelection,
        meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> DependencyValidationResult<Cow<'_, PrebuiltManifestSummaryV1>> {
        let summary = match self.summary.get() {
            Some(summary) if summary.target_selection() == target => Cow::Borrowed(summary),
            previous => {
                let decoded = probe_prebuilt_manifest_summary(&self.bytes, limits, target)
                    .map_err(|source| {
                        Box::new(ExplicitDependencyValidationError::Summary {
                            input: self.input.clone(),
                            source: Box::new(source),
                        })
                    })?;
                if previous.is_none() {
                    Cow::Borrowed(self.summary.get_or_init(|| decoded))
                } else {
                    Cow::Owned(decoded)
                }
            }
        };
        if let Some(meter) = meter {
            let snapshot = sha256(&self.bytes);
            meter
                .observe_artifact_snapshot(&summary, snapshot)
                .and_then(|()| {
                    meter.charge_artifact_decode(
                        SlibClosureDecodePurposeV1::GraphSummary,
                        summary.artifact_fingerprint(),
                        snapshot,
                        summary.decode_usage(),
                    )
                })
                .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))?;
        }
        Ok(summary)
    }
}

impl LoadedExplicitDependencyInputs {
    pub(in crate::request::preflight) fn contains_explicit_core(
        &self,
        target: ValidatedLirTargetSelection,
        mut meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> DependencyValidationResult<bool> {
        let mut contains_core = false;
        for artifact in &self.artifacts {
            let summary = artifact.summary(self.limits, target, meter.as_deref_mut())?;
            contains_core |= summary.cone().identity() == ConeIdentity::CORE;
        }
        Ok(contains_core)
    }
}
