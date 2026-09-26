//! Publication metadata from the same checked semantic and object inputs.

use super::*;
impl CrossConeArtifactSummary {
    pub(crate) fn from_layout(
        manifest: &crate::BootstrapManifest,
        production: &crate::SingleConeProductionCodeProjectionV1,
        link_object_count: usize,
        target: ValidatedLirTargetSelection,
    ) -> Self {
        let cone = manifest.cone();
        let semantic = manifest.semantic_fingerprints();

        CrossConeArtifactSummary {
            artifact_fingerprint: manifest.artifact_fingerprint(),
            coordinate: cone.coordinate().clone(),
            identity: cone.identity(),
            kind: cone.kind(),
            source_form: cone.source_form(),
            target_selection: target,
            profile: manifest.compatibility().artifact_profile().clone(),
            direct_dependencies: manifest.direct_dependencies().to_vec(),
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
