//! Reader summaries from complete semantic and Link-object data.

use scoop_lir::CBridgeToolchainProfileV1;

use super::*;
use crate::CrossConeClosureArtifactSlotV1;

mod errors;
mod summary;
pub(crate) fn capture_layout_publication(
    artifact: &crate::PhysicalImportsReplayedCrossConeLayoutSections,
    symbols: &crate::ReplayedLayoutLinkSymbolUsesV1,
    target: ValidatedLirTargetSelection,
) -> Result<CrossConeArtifactSummary, Error> {
    Ok(CrossConeArtifactSummary::from_layout(
        artifact.manifest(),
        symbols.production_projection(),
        symbols.link_objects().members().len(),
        target,
        crate::TestDirectory::from_manifest(artifact.manifest()).map_err(Error::TestDirectory)?,
    ))
}
pub use errors::CrossConeLayoutArtifactValidationError;

use CrossConeLayoutArtifactValidationError as Error;

/// Reads the common semantic sections once, then checks the actual Link objects.
pub fn read_cross_cone_layout_artifact_summary(
    final_bytes: &[u8],

    current: ConeIdentity,
    direct: &[ConeIdentity],
    dependency_first: &[&[u8]],
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<CrossConeArtifactSummary, Error> {
    let complete = crate::read_cross_cone_layout_artifact_closure(
        crate::CrossConeArtifactClosureInput::completed(
            current,
            target,
            direct.to_vec(),
            dependency_first.to_vec(),
            final_bytes,
        ),
        c_bridge_profile,
    )?;
    let (artifact, symbols) = complete
        .artifact(current)
        .ok_or(Error::MissingCurrentArtifact)?;
    capture_layout_publication(artifact, symbols, target)
}
