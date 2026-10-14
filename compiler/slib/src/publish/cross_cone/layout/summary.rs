//! Publication metadata from the same checked semantic and object inputs.

use super::*;
impl CrossConeArtifactSummary {
    pub(crate) fn from_layout(
        manifest: &crate::BootstrapManifest,
        production: &crate::SingleConeProductionCodeProjectionV1,
        link_object_count: usize,
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
            compile_summary: CompileViewSummaryV1::new(semantic),
            link_summary: LinkViewSummaryV1 {
                distribution: production.distribution(),
                output: production.output().clone(),
                image_owner_member: production.image_owner_member(),
                link_object_count,
                semantic_fingerprints: semantic,
            },
        }
    }
}
