//! Digest relationships used by valid and negative registration fixtures.

use super::*;

pub(super) fn digest_plan(
    foundation: &ConeLirFoundation,
    callables: &[CallableArtifacts],
) -> DigestFinalizationPlanV1 {
    let mut nodes = callables
        .iter()
        .map(|item| {
            DigestNodeV1::new(
                DigestNodeKey::object_definition(item.body_definition.primary.id()),
                Vec::new(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let image_inputs = nodes.iter().map(DigestInputRefV1::from_node).collect();
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(foundation.producer()),
            image_inputs,
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
