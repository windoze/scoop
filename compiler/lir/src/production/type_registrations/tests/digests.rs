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
        let descriptor_source = DigestNodeId::from_key(&descriptor_key).unwrap();
        let layout_key = DigestNodeKey::layout(artifacts.layout.id());
        let layout_source = DigestNodeId::from_key(&layout_key).unwrap();

        let registration_present = !(options.omit_last_registration && index == 1);
        let descriptor = DigestNodeV1::new(
            descriptor_key,
            Vec::new(),
            if registration_present && !(options.omit_descriptor_patch && index == 0) {
                vec![DigestPatchIntentKey::new(
                    descriptor_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::DescriptorDefinition,
                )]
            } else {
                Vec::new()
            },
        )
        .unwrap();
        let layout = (!options.omit_first_layout || index != 0).then(|| {
            DigestNodeV1::new(
                layout_key,
                Vec::new(),
                if registration_present && !(options.omit_layout_patch && index == 0) {
                    vec![DigestPatchIntentKey::new(
                        layout_source,
                        artifacts.registration_definition.id(),
                        DefinitionAtomRole::Primary,
                        DigestSemanticFieldRole::Layout,
                    )]
                } else {
                    Vec::new()
                },
            )
            .unwrap()
        });

        nodes.push(descriptor.clone());
        if let Some(layout) = &layout {
            nodes.push(layout.clone());
        }
        if !registration_present {
            continue;
        }

        let object_key = DigestNodeKey::object_definition(artifacts.registration_primary.id());
        let object_source = DigestNodeId::from_key(&object_key).unwrap();
        let object = DigestNodeV1::new(
            object_key,
            if options.registration_object_input && index == 0 {
                vec![DigestInputRefV1::from_node(layout.as_ref().unwrap())]
            } else {
                Vec::new()
            },
            if options.registration_object_patch && index == 0 {
                vec![DigestPatchIntentKey::new(
                    object_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::CallableBodyDefinition,
                )]
            } else {
                Vec::new()
            },
        )
        .unwrap();
        let strong_key = DigestNodeKey::strong_registration(artifacts.registration_definition.id());
        let strong_source = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = vec![DigestInputRefV1::from_node(&object)];
        if !options.omit_descriptor_input || index != 0 {
            inputs.push(DigestInputRefV1::from_node(&descriptor));
        }
        if let Some(layout) = &layout {
            inputs.push(DigestInputRefV1::from_node(layout));
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if options.omit_registration_patch && index == 0 {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_source,
                    artifacts.registration_definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(object);
        nodes.push(strong);
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}
