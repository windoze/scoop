use scoop_hir::{
    DefaultExpressionV1, ExportDefaultBodyV1, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceV1, ExportDefaultTemplateV1,
};

use super::*;

pub(super) fn replace(
    current: &mut CallableSourceSurface,
    origin: ExportDefinitionSourceV1,
    parameter: bool,
) -> usize {
    let interface = &current.interface;
    let template = &interface.default_templates().records()[0];
    let value = template.body().value();
    let references = template.references();
    let mut types = references.types().to_vec();
    types.push(ExportDefaultReferenceV1::new(
        template.result().clone(),
        origin.clone(),
    ));
    types.sort_unstable();
    types.dedup();
    let references = ExportDefaultReferenceSetV1::try_new(
        references.callables().to_vec(),
        references.constructors().to_vec(),
        types,
        references.globals().to_vec(),
        references.singleton_values().to_vec(),
        references.fields().to_vec(),
    )
    .unwrap();
    let replacement = ExportDefaultTemplateV1::try_new(
        template.key(),
        template.definition_root(),
        template.definition_path().clone(),
        template.locals().clone(),
        ExportDefaultBodyV1::try_new(
            template.body().statements().to_vec(),
            DefaultExpressionV1::try_new(
                value.kind().clone(),
                template.result().clone(),
                origin.clone(),
            )
            .unwrap(),
        )
        .unwrap(),
        template.result().clone(),
        template.allows_suspend(),
        template.type_parameters().clone(),
        template.receiver().clone(),
        template.value_parameters().clone(),
        references,
        template.definition_origin().clone(),
    )
    .unwrap();
    let mut locations = interface.definition_sources().sources().to_vec();
    locations.push(origin.clone());
    locations.sort();
    locations.dedup();
    let index = locations.binary_search(&origin).unwrap();
    let source_interfaces = if parameter {
        let source = interface.source_interfaces().get(current.owner).unwrap();
        let original = &source.parameters().parameters()[0];
        CanonicalCallableSourceInterfacesV1::try_new(vec![
            CallableSourceInterfaceV1::try_new(
                current.owner,
                CanonicalCallableSourceParametersV1::try_new(vec![CallableSourceParameterV1::new(
                    original.name().clone(),
                    original.value_type().clone(),
                    original.calling().clone(),
                    origin,
                )])
                .unwrap(),
            )
            .unwrap(),
        ])
        .unwrap()
    } else {
        interface.source_interfaces().clone()
    };
    current.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        source_interfaces,
        CanonicalExportDefaultTemplatesV1::try_new(vec![replacement]).unwrap(),
        interface.constants().clone(),
        CanonicalExportDefinitionSourcesV1::try_new(locations).unwrap(),
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
    );
    index
}
