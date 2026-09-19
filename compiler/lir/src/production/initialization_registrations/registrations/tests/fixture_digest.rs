//! Digest relationships used by valid and negative registration fixtures.

use super::*;

pub(super) fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    cell: &DefinitionArtifacts,
    descriptor: &DefinitionArtifacts,
    registration: &DefinitionArtifacts,
    storages: &[DefinitionArtifacts; 2],
    callables: &[CallableArtifacts],
    options: Options,
) -> StrongDigestFinalizationPlanV1 {
    let auxiliary_input = options.cell_object_input.then(|| {
        DigestNodeV1::new(
            DigestNodeKey::source_signature(callables[0].body.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    let cell_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(cell.primary.id()),
        auxiliary_input
            .as_ref()
            .map(DigestInputRefV1::from_node)
            .into_iter()
            .collect(),
        Vec::new(),
    )
    .unwrap();
    let descriptor_object = (!options.omit_descriptor_primary && !options.omit_descriptor_object)
        .then(|| {
            DigestNodeV1::new(
                DigestNodeKey::object_definition(descriptor.primary.id()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap()
        });

    let unit_registration_present = !options.omit_unit_registration;
    let registration_object = unit_registration_present.then(|| {
        DigestNodeV1::new(
            DigestNodeKey::object_definition(registration.primary.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    let callable_body_nodes = callables
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let key = DigestNodeKey::object_definition(item.body_definition.primary.id());
            let source = DigestNodeId::from_key(&key).unwrap();
            let is_gateway = !options.lazy && index == 2;
            let patches = if unit_registration_present
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

    let mut nodes = vec![cell_object.clone()];
    nodes.extend(auxiliary_input);
    nodes.extend(descriptor_object.clone());
    nodes.extend(registration_object.clone());
    nodes.extend(callable_body_nodes.iter().cloned());
    let mut image_inputs = Vec::new();
    for (index, storage) in storages.iter().enumerate() {
        if options.omit_storage_registration && index == 0 {
            continue;
        }
        let strong = DigestNodeV1::new(
            DigestNodeKey::strong_registration(storage.definition.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    for (index, callable) in callables.iter().enumerate() {
        if options.omit_callable_registration && index == 0 {
            continue;
        }
        let strong = DigestNodeV1::new(
            DigestNodeKey::strong_registration(callable.registration.definition.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    if unit_registration_present {
        let strong_key = DigestNodeKey::strong_registration(registration.definition.id());
        let strong_source = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = vec![
            DigestInputRefV1::from_node(registration_object.as_ref().unwrap()),
            DigestInputRefV1::from_node(&cell_object),
        ];
        if !options.omit_descriptor_input {
            if let Some(descriptor) = &descriptor_object {
                inputs.push(DigestInputRefV1::from_node(descriptor));
            }
        }
        if !options.lazy {
            inputs.push(DigestInputRefV1::from_node(&callable_body_nodes[2]));
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if options.omit_registration_patch {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_source,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(foundation.producer()),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
