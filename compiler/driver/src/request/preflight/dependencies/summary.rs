use std::borrow::Cow;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{PrebuiltManifestSummaryV1, probe_prebuilt_manifest_summary};

use super::*;

impl LoadedExplicitDependencyArtifact {
    pub(super) fn summary(
        &self,

        target: ValidatedLirTargetSelection,
    ) -> DependencyValidationResult<Cow<'_, PrebuiltManifestSummaryV1>> {
        let summary = match self.summary.get() {
            Some(summary) if summary.target_selection() == target => Cow::Borrowed(summary),
            previous => {
                let decoded =
                    probe_prebuilt_manifest_summary(&self.bytes, target).map_err(|source| {
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

        Ok(summary)
    }
}

impl LoadedExplicitDependencyInputs {
    pub(in crate::request::preflight) fn contains_explicit_core(
        &self,
        target: ValidatedLirTargetSelection,
    ) -> DependencyValidationResult<bool> {
        let mut contains_core = false;
        for artifact in &self.artifacts {
            let summary = artifact.summary(target)?;
            contains_core |= summary.cone().identity() == ConeIdentity::CORE;
        }
        Ok(contains_core)
    }
}
