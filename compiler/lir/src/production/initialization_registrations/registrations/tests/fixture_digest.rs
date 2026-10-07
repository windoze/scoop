//! Digest relationships used by valid and negative registration fixtures.

use super::*;

pub(super) fn digest_plan(
    foundation: &ConeLirFoundation,
    registration: &DefinitionArtifacts,
    callables: &[CallableArtifacts],
    options: Options,
) -> DigestFinalizationPlanV1 {
    let unit_registration_present = !options.omit_unit_registration;
    let registration_primary_present =
        unit_registration_present && !options.omit_registration_primary;
    let callable_body_nodes = callables
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let key = DigestNodeKey::object_definition(item.body_definition.primary.id());
            let source = DigestNodeId::from_key(&key).unwrap();
            let is_gateway = !options.lazy && index == 2;
            let patches = if registration_primary_present
                && ((is_gateway && !options.omit_gateway_patch)
                    || (!is_gateway
                        && options.unexpected_lazy_gateway_patch
                        && item.body.id() == callables[1].body.id()))
            {
                vec![DigestPatchIntentKey::new(
                    source,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::GatewayDefinition,
                )]
            } else {
                Vec::new()
            };
            DigestNodeV1::new(key, Vec::new(), patches).unwrap()
        })
        .collect::<Vec<_>>();

    let mut nodes = callable_body_nodes;
    let image_inputs = nodes
        .iter()
        .filter(|node| !node.patch_intents().is_empty())
        .map(DigestInputRefV1::from_node)
        .collect();
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(foundation.producer()),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
