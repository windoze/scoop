//! Complete type-registration digest DAG fixtures.

use super::*;

pub(super) fn digest_plan(
    foundation: &ConeLirFoundation,
    types: &[TypeArtifacts; 2],
    options: Options,
) -> DigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    for (index, artifacts) in types.iter().enumerate() {
        let descriptor_key = DigestNodeKey::object_definition(artifacts.descriptor_primary.id());

        let layout_key = DigestNodeKey::layout(artifacts.layout.id());

        let registration_present = !(options.omit_last_registration && index == 1);
        let descriptor = DigestNodeV1::new(descriptor_key, Vec::new()).unwrap();
        let layout = (!options.omit_first_layout || index != 0)
            .then(|| DigestNodeV1::new(layout_key, Vec::new()).unwrap());

        nodes.push(descriptor.clone());
        if let Some(layout) = &layout {
            nodes.push(layout.clone());
        }
        if !registration_present {
            continue;
        }

        image_inputs.push(DigestInputRefV1::from_node(&descriptor));
        image_inputs.extend(layout.iter().map(DigestInputRefV1::from_node));
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
