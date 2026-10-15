//! Publication metadata from the saved manifest.

use super::*;
impl CrossConeArtifactSummary {
    pub(crate) fn from_layout(
        manifest: &crate::BootstrapManifest,
        target: ValidatedLirTargetSelection,
        test_directory: Option<crate::TestDirectory>,
    ) -> Self {
        let semantic = manifest.semantic_fingerprints();

        CrossConeArtifactSummary {
            manifest: crate::ArtifactManifestSummaryV1::new(
                manifest.cone().clone(),
                manifest.direct_dependencies().to_vec(),
                target,
                manifest.compatibility().artifact_profile().clone(),
                semantic,
                manifest.artifact_fingerprint(),
                test_directory,
            ),
        }
    }
}
