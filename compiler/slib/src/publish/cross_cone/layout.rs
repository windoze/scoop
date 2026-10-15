//! Reader summaries from shared semantic sections and saved definition metadata.

use super::*;
use crate::CrossConeClosureArtifactSlotV1;

mod errors;
mod summary;
pub use errors::CrossConeLayoutArtifactValidationError;

use CrossConeLayoutArtifactValidationError as Error;

/// Read semantic and definition metadata without inspecting object contents.
pub fn read_cross_cone_layout_artifact_summary(
    final_bytes: &[u8],
    current: ConeIdentity,
    direct: &[ConeIdentity],
    dependency_first: &[&[u8]],
    target: ValidatedLirTargetSelection,
) -> Result<CrossConeArtifactSummary, Error> {
    let complete = crate::read_cross_cone_layout_artifact_closure(
        crate::CrossConeArtifactClosureInput::completed(
            current,
            target,
            direct.to_vec(),
            dependency_first.to_vec(),
            final_bytes,
        ),
    )?;
    let (_, definitions) = complete
        .artifact(current)
        .ok_or(Error::MissingCurrentArtifact)?;
    Ok(definitions.summary().clone())
}
