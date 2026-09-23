//! Link inputs borrowed from the validated Compile view of the same archive.

use super::*;
use crate::{CrossConeSemanticsStrongProfile, PublishViewMismatchError, ValidatedCompileArtifact};

pub fn validate_cross_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    validate_compile_view(&graph, compile)?;
    cross_cone::validate_cross_cone_strong_link_parts(
        graph,
        compile.production().hir_interface(),
        expected_external_bridges,
        compile.production().lir_cross_cone(),
        dependency_owners,
        c_bridge_profile,
    )
}

pub fn validate_self_describing_cross_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    validate_self_describing_cross_cone_strong_link_artifact_with_authorities(
        graph,
        std::iter::empty(),
        compile,
        dependency_owners,
        c_bridge_profile,
    )
}

pub(crate) fn validate_self_describing_cross_cone_strong_link_artifact_with_authorities<
    'input,
    'authority,
>(
    graph: ValidatedGraphArtifact<'input>,
    external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
    compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    validate_compile_view(&graph, compile)?;
    cross_cone::validate_self_describing_cross_cone_strong_link_parts(
        graph,
        external_authorities,
        compile.production().hir_interface(),
        compile.production().lir_cross_cone(),
        dependency_owners,
        c_bridge_profile,
    )
}

fn validate_compile_view(
    graph: &ValidatedGraphArtifact<'_>,
    compile: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
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
