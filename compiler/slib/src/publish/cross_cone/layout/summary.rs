//! Publication metadata from the same checked semantic and object inputs.

use super::*;
use crate::{ReplayedLayoutLinkSymbolUsesV1, layout_compile_closure::LayoutPublicationParts};

pub(super) fn capture(
    parts: LayoutPublicationParts<'_>,
    link: &ReplayedLayoutLinkSymbolUsesV1,
    target: ValidatedLirTargetSelection,
) -> PublishableCrossConeArtifact {
    let manifest = parts.manifest;
    let cone = manifest.cone();
    let semantic = manifest.semantic_fingerprints();
    let production = link.production_projection();

    PublishableCrossConeArtifact {
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
            link_object_count: link.link_objects().members().len(),
            semantic_fingerprints: semantic,
        },
    }
}
