use scoop_identity::CanonicalIdentifier;

use super::super::super::body::expression_test_support::Fixture;
use super::*;
use crate::{
    CallableParameterCallingV1, CallableSourceInterfaceV1, CallableSourceParameterV1,
    CanonicalCallableSourceInterfacesV1, CanonicalCallableSourceParametersV1,
};

use super::super::tests::template;

mod contracts;

#[test]
fn exact_source_template_closure_is_accepted() {
    let fixture = Fixture::new();
    let template = template(&fixture, 0);
    let key = template.key();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap();
    let sources = source_table(
        &fixture,
        vec![CallableParameterCallingV1::Default { template: key }],
    );

    assert_eq!(templates.validate_source_closure(&sources), Ok(()));
}

#[test]
fn template_requires_an_owner_source_interface_and_parameter() {
    let fixture = Fixture::new();
    let template = template(&fixture, 0);
    let key = template.key();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap();
    let empty_sources = CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        templates.validate_source_closure(&empty_sources),
        Err(
            ExportDefaultTemplateSourceClosureValidationError::MissingSourceInterface {
                template_index: 0,
                owner: key.owner(),
            }
        )
    );

    let sources = source_table(&fixture, Vec::new());
    assert_eq!(
        templates.validate_source_closure(&sources),
        Err(
            ExportDefaultTemplateSourceClosureValidationError::ParameterOutOfRange {
                template_index: 0,
                key,
                arity: 0,
            }
        )
    );
}

#[test]
fn template_must_be_referenced_by_its_exact_source_parameter() {
    let fixture = Fixture::new();
    let template = template(&fixture, 0);
    let key = template.key();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap();
    let sources = source_table(&fixture, vec![CallableParameterCallingV1::Required]);

    assert_eq!(
        templates.validate_source_closure(&sources),
        Err(
            ExportDefaultTemplateSourceClosureValidationError::ParameterReference {
                template_index: 0,
                key,
                actual: None,
            }
        )
    );
}

#[test]
fn every_source_default_reference_must_have_a_template() {
    let fixture = Fixture::new();
    let key = ExportDefaultTemplateKeyV1::new(
        scoop_identity::CallableTemplateOrigin::Function(fixture.function),
        0,
    );
    let templates = CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap();
    let sources = source_table(
        &fixture,
        vec![CallableParameterCallingV1::Default { template: key }],
    );

    assert_eq!(
        templates.validate_source_closure(&sources),
        Err(
            ExportDefaultTemplateSourceClosureValidationError::MissingTemplate {
                source_index: 0,
                owner: key.owner(),
                parameter_position: 0,
                key,
            }
        )
    );
}

#[test]
fn required_parameters_do_not_create_template_obligations() {
    let fixture = Fixture::new();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap();
    let sources = source_table(&fixture, vec![CallableParameterCallingV1::Required]);

    assert_eq!(templates.validate_source_closure(&sources), Ok(()));
}

fn source_table(
    fixture: &Fixture,
    calling: Vec<CallableParameterCallingV1>,
) -> CanonicalCallableSourceInterfacesV1 {
    let parameters = calling
        .into_iter()
        .enumerate()
        .map(|(index, calling)| {
            let name = format!("p{index}");
            CallableSourceParameterV1::new(
                CanonicalIdentifier::new(&name).unwrap(),
                fixture.value_type(),
                calling,
                fixture.origin(),
            )
        })
        .collect();
    let source = CallableSourceInterfaceV1::try_new(
        scoop_identity::CallableTemplateOrigin::Function(fixture.function),
        CanonicalCallableSourceParametersV1::try_new(parameters).unwrap(),
    )
    .unwrap();
    CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap()
}
