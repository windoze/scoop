use scoop_slib::ArtifactManifestSummaryV1;

use super::*;

impl LoadedExplicitDependencyArtifact {
    pub(super) fn summary(&self) -> DependencyValidationResult<&ArtifactManifestSummaryV1> {
        self.artifact.summary().map_err(|source| {
            Box::new(ExplicitDependencyValidationError::Summary {
                input: self.input.clone(),
                source: Box::new(ArtifactManifestSummaryError::Envelope(source)),
            })
        })
    }
}

impl LoadedExplicitDependencyInputs {
    pub(in crate::request::preflight) fn contains_explicit_core(
        &self,
    ) -> DependencyValidationResult<bool> {
        let mut contains_core = false;
        for artifact in &self.artifacts {
            let summary = artifact.summary()?;
            contains_core |= summary.cone().identity() == ConeIdentity::CORE;
        }
        Ok(contains_core)
    }
}
