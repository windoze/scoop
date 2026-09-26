//! Link inputs borrowed from the validated Compile view of the same archive.

use super::*;
use crate::{CrossConeSemanticsStrongProfile, PublishViewMismatchError, ValidatedCompileArtifact};

pub fn validate_self_describing_cross_cone_strong_link_artifact(
    mut graph: ValidatedGraphArtifact<'_>,
    compile: &ValidatedCompileArtifact<CrossConeSemanticsStrongProfile>,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact, StrongLinkArtifactValidationError> {
    validate_compile_view(&graph, compile)?;
    let metadata = crate::compile_sections::decode_compile_metadata_envelopes(
        &mut graph,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
    )
    .map_err(|error| StrongLinkArtifactValidationError::SharedMetadata(Box::new(error)))?;
    let sections = decode_cross_cone_link_only(graph, &metadata)
        .map_err(|error| StrongLinkArtifactValidationError::Decode(Box::new(error)))?;
    validate_cross_cone_link_from_compile(sections, compile, dependency_owners, c_bridge_profile)
}

fn validate_compile_view(
    graph: &ValidatedGraphArtifact<'_>,
    compile: &ValidatedCompileArtifact<CrossConeSemanticsStrongProfile>,
) -> Result<(), StrongLinkArtifactValidationError> {
    use PublishViewMismatchError as Error;
    let mismatch =
        if graph.identity() != compile.identity() || graph.coordinate() != compile.coordinate() {
            Error::Cone
        } else if graph.artifact_fingerprint() != compile.artifact_fingerprint() {
            Error::ArtifactFingerprint
        } else if graph.kind() != compile.kind() || graph.source_form() != compile.source_form() {
            Error::ConeShape
        } else if graph.target_selection() != compile.target_selection() {
            Error::TargetSelection
        } else if graph.compatibility() != compile.compatibility() {
            Error::Compatibility
        } else {
            return Ok(());
        };
    Err(StrongLinkArtifactValidationError::CompileView(mismatch))
}
