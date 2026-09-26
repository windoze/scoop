//! Publication from one shared semantic and Link-object validation.

use scoop_lir::CBridgeToolchainProfileV1;

use super::*;
use crate::{
    CrossConeClosureArtifactSlotV1, DecodedCrossConeLayoutCompileClosure,
    DecodedCrossConeLayoutCompileSections, DecodedSlibEnvelope,
};

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
    let mut dependencies = Vec::new();
    for (index, bytes) in dependency_first.iter().enumerate() {
        dependencies.push(decode(
            bytes,
            target,
            CrossConeClosureArtifactSlotV1::Dependency(index),
        )?);
    }
    let front = decode(final_bytes, target, CrossConeClosureArtifactSlotV1::Current)?;
    let semantic = DecodedCrossConeLayoutCompileClosure::with_current_artifact(
        current,
        target,
        direct.to_vec(),
        dependencies,
        front,
    )
    .replay_semantics()
    .map_err(|source| Error::Semantic {
        source: Box::new(source),
    })?;
    let complete = semantic
        .replay_link_symbol_uses(c_bridge_profile)
        .map_err(|source| Error::Physical {
            source: Box::new(source),
        })?;
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

fn decode<'input>(
    bytes: &'input [u8],

    target: ValidatedLirTargetSelection,
    slot: CrossConeClosureArtifactSlotV1,
) -> Result<DecodedCrossConeLayoutCompileSections<'input>, Error> {
    DecodedSlibEnvelope::open(bytes, target)
        .map_err(|source| Error::Envelope {
            slot,
            source: Box::new(source),
        })?
        .validate_graph()
        .map_err(|source| Error::Graph {
            slot,
            source: Box::new(source),
        })?
        .decode_cross_cone_layout_link_sections()
        .map_err(|source| Error::LinkSections {
            slot,
            source: Box::new(source),
        })?
        .into_shared_sections()
        .map_err(|source| Error::Resource { source })
}
