use super::*;
use crate::{
    DefaultTemplateContractViewV1, DefaultTemplateDeclarationContractError as ContractError,
    DefaultTemplateDeclarationContractV1, DefaultTemplateProviderParameterV1,
};
use scoop_wire::WirePath;

fn validate(
    template: &ExportDefaultTemplateV1,
    callable: &CallableInterfaceRecordV1,
    authority: &Authority,
) -> Result<(), ContractError<AuthorityError>> {
    let provider = DefaultTemplateDeclarationContractV1::new(
        template.definition_root().declaration(),
        authority.provider_shape,
        DefaultTemplateProviderParameterV1::try_new(
            &authority.provider_parameters,
            authority.provider_position,
        )
        .unwrap(),
        0,
        authority
            .provider_receiver
            .as_ref()
            .map(std::borrow::Cow::Borrowed),
        Effect::Ordinary,
    );
    let publisher = DefaultTemplateDeclarationContractV1::new(
        callable.declaration(),
        DefaultTemplateProviderShapeV1::try_new(0, callable.type_parameters().len_u32()).unwrap(),
        DefaultTemplateProviderParameterV1::try_new(
            callable.parameters(),
            template.key().parameter_position(),
        )
        .unwrap(),
        0,
        None,
        callable.effects().execution(),
    );
    let mut shapes = Fixture::new().authority();
    DefaultTemplateContractViewV1::from(template).validate(
        &publisher,
        &provider,
        &mut shapes,
        &WirePath::root(),
    )
}

#[test]
fn shared_contract_preserves_raw_parameters_when_distinct_binders_collapse() {
    let mut fixture = Fixture::new();
    let (callable, _, authority) = publishing(&mut fixture, vec![binder(0, 0); 2]);
    let template = collapsed(&fixture);

    validate(&template, &callable, &authority).unwrap();
    let mut result = template.clone();
    result.result = binder(0, 0);
    assert!(matches!(
        validate(&result, &callable, &authority),
        Err(ContractError::ResultType)
    ));
    let prefix = fixture.with_parameter_local(template, binder(1, 0), false);
    assert!(matches!(
        validate(&prefix, &callable, &authority),
        Err(ContractError::Prefix(
            crate::TemplateValueParameterSemanticValidationError::LocalType { position: 0, .. }
        ))
    ));
}

#[test]
fn shared_contract_checks_the_complete_parameter_tail_and_the_selected_position() {
    let mut fixture = Fixture::new();
    let (callable, _, mut authority) =
        publishing(&mut fixture, vec![binder(0, 0), binder(0, 0), binder(0, 1)]);
    let template = collapsed(&fixture);

    assert!(matches!(
        validate(&template, &callable, &authority),
        Err(ContractError::ParameterArity)
    ));
    authority.provider_parameters = source_shapes(vec![binder(0, 0), binder(1, 0), binder(0, 0)]);
    assert!(matches!(
        validate(&template, &callable, &authority),
        Err(ContractError::ParameterType { index: 2 })
    ));
    authority.provider_position = 0;
    assert!(matches!(
        validate(&template, &callable, &authority),
        Err(ContractError::ParameterPosition)
    ));
}
