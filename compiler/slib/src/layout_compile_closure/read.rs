//! Shared artifact reading for publication and downstream compilation.

use super::*;
use crate::{
    CrossConeArtifactClosureInput, CrossConeClosureArtifactSlotV1,
    CrossConeLayoutArtifactValidationError as Error, DecodedSlibEnvelope,
};

pub fn read_cross_cone_layout_artifact_closure(
    input: CrossConeArtifactClosureInput<'_>,
    c_bridge_profile: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<LinkSymbolsReplayedCrossConeLayoutClosure, Error> {
    let mut dependencies = Vec::new();
    for (index, bytes) in input.dependency_first.iter().enumerate() {
        dependencies.push(decode(
            bytes,
            input.target,
            CrossConeClosureArtifactSlotV1::Dependency(index),
        )?);
    }
    let decoded = if let Some(bytes) = input.current_artifact {
        let current = decode(bytes, input.target, CrossConeClosureArtifactSlotV1::Current)?;
        DecodedCrossConeLayoutCompileClosure::with_current_artifact(
            input.current,
            input.target,
            input.direct,
            dependencies,
            current,
        )
    } else {
        DecodedCrossConeLayoutCompileClosure::new(
            input.current,
            input.target,
            input.direct,
            dependencies,
        )
    };
    decoded
        .replay_semantics()
        .map_err(|source| Error::Semantic {
            source: Box::new(source),
        })?
        .replay_link_symbol_uses(c_bridge_profile)
        .map_err(|source| Error::Physical {
            source: Box::new(source),
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
