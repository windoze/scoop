use super::*;
use scoop_hir::*;

#[test]
fn reader_requires_source_parameter_protocols_for_support_callables() {
    let mut fixture = support(false);
    replace(
        &mut fixture,
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
    );
    let bytes = fixture.artifact();
    assert!(
        matches!(validate_until_type_alias(&bytes).validate_source_interfaces(vec![]),
        Err(CrossConeHirSourceInterfaceSurfaceError::SourceInterfaces(
            CallableSourceInterfaceSetSemanticValidationError::MissingSourceInterface(id)
        )) if id == fixture.owner)
    );
}

#[test]
fn reader_checks_the_actual_body_reference_closure_of_support_defaults() {
    let mut fixture = support(true);
    default_fixture::replace_references(
        &mut fixture,
        ExportDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
            .unwrap(),
    );
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract(
        crate::CrossConeHirDefaultProviderContractError::Template { source, .. },
    )) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("a support default must keep its exact body references");
    };
    assert!(matches!(
        *source,
        crate::CrossConeHirDefaultProviderContractError::ReferenceClosure(_)
    ));
}

fn support(default: bool) -> CallableSourceSurface {
    let mut fixture = if default {
        default_fixture::fixture(default_fixture::Case::Defined)
    } else {
        CallableSourceSurface::new(SourceInterfaceCase::Complete)
    };
    let i = &fixture.interface;
    let callables = CanonicalCallableInterfacesV1::with_support(
        vec![],
        vec![
            i.callable_interfaces()
                .get(fixture.owner)
                .unwrap()
                .declaration_data()
                .clone(),
        ],
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        i.public_bindings().clone(),
        i.nominal_interfaces().clone(),
        callables,
        i.property_interfaces().clone(),
        i.type_aliases().clone(),
        i.source_interfaces().clone(),
        i.default_templates().clone(),
        i.constants().clone(),
        i.definition_sources().clone(),
        i.external_references().clone(),
        i.generic_callable_bodies().clone(),
        i.generic_initializations().clone(),
    );
    fixture
}

fn replace(
    fixture: &mut CallableSourceSurface,
    protocols: CanonicalCallableSourceInterfacesV1,
    templates: CanonicalExportDefaultTemplatesV1,
) {
    let i = &fixture.interface;
    let sources = CanonicalExportDefinitionSourcesV1::from_interface_parts(
        i.type_aliases(),
        &protocols,
        &templates,
        i.constants(),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        i.public_bindings().clone(),
        i.nominal_interfaces().clone(),
        i.callable_interfaces().clone(),
        i.property_interfaces().clone(),
        i.type_aliases().clone(),
        protocols,
        templates,
        i.constants().clone(),
        sources,
        i.external_references().clone(),
        i.generic_callable_bodies().clone(),
        i.generic_initializations().clone(),
    );
}
