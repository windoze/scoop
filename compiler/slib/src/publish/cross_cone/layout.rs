//! Publication from one shared semantic and Link-object validation.

use scoop_lir::CBridgeToolchainProfileV1;

use super::*;
use crate::CrossConeClosureArtifactSlotV1;

mod errors;
mod summary;
pub use errors::CrossConeLayoutArtifactValidationError;

use CrossConeLayoutArtifactValidationError as Error;

/// Reads the common semantic sections once, then checks the actual Link objects.
#[allow(clippy::too_many_arguments)]
pub fn validate_publishable_cross_cone_layout_artifact(
    final_bytes: &[u8],

    current: ConeIdentity,
    direct: &[ConeIdentity],
    dependency_first: &[&[u8]],
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishableCrossConeArtifact, Error> {
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
    Ok(summary::capture(
        artifact.publication_parts(),
        symbols,
        target,
    ))
}

/// Atomically publishes the final archive after its shared validation succeeds.
#[allow(clippy::too_many_arguments)]
pub fn publish_cross_cone_layout_artifact(
    final_bytes: &[u8],
    destination: &Path,

    current: ConeIdentity,
    direct: &[ConeIdentity],
    dependency_first: &[&[u8]],
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<PublishedCrossConeArtifact, CrossConeArtifactPublishError> {
    atomic::publish(final_bytes, destination, |bytes| {
        validate_publishable_cross_cone_layout_artifact(
            bytes,
            current,
            direct,
            dependency_first,
            target,
            c_bridge_profile,
        )
        .map_err(|source| CrossConeArtifactPublishError::LayoutValidation(Box::new(source)))
    })
}
