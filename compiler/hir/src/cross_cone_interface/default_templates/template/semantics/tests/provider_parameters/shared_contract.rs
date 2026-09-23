use super::*;
use crate::{
    DefaultTemplateContractViewV1, DefaultTemplateDeclarationContractError as ContractError,
    DefaultTemplateDeclarationContractV1, DefaultTemplateProviderParameterV1,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

fn validate(
    template: &ExportDefaultTemplateV1,
    callable: &CallableInterfaceRecordV1,
    authority: &Authority,
    meter: &mut BudgetMeter,
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
        meter,
        &WirePath::root(),
    )
}

#[test]
fn shared_contract_preserves_raw_parameters_when_distinct_binders_collapse() {
    let mut fixture = Fixture::new();
    let (callable, _, authority) = publishing(&mut fixture, vec![binder(0, 0); 2]);
    let template = collapsed(&fixture);
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    validate(&template, &callable, &authority, &mut meter).unwrap();
    let mut result = template.clone();
    result.result = binder(0, 0);
    assert!(matches!(
        validate(&result, &callable, &authority, &mut meter),
        Err(ContractError::ResultType)
    ));
    let prefix = fixture.with_parameter_local(template, binder(1, 0), false);
    assert!(matches!(
        validate(&prefix, &callable, &authority, &mut meter),
        Err(ContractError::Prefix(
            crate::MeteredTemplateValueParameterSemanticValidationError::LocalType { position: 0 }
        ))
    ));
}

#[test]
fn shared_contract_checks_the_complete_parameter_tail_and_the_selected_position() {
    let mut fixture = Fixture::new();
    let (callable, _, mut authority) =
        publishing(&mut fixture, vec![binder(0, 0), binder(0, 0), binder(0, 1)]);
    let template = collapsed(&fixture);
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    assert!(matches!(
        validate(&template, &callable, &authority, &mut meter),
        Err(ContractError::ParameterArity)
    ));
    authority.provider_parameters = source_shapes(vec![binder(0, 0), binder(1, 0), binder(0, 0)]);
    assert!(matches!(
        validate(&template, &callable, &authority, &mut meter),
        Err(ContractError::ParameterType { index: 2 })
    ));
    authority.provider_position = 0;
    assert!(matches!(
        validate(&template, &callable, &authority, &mut meter),
        Err(ContractError::ParameterPosition)
    ));
}

#[test]
fn shared_contract_uses_the_same_remaining_resource_budget() {
    let mut fixture = Fixture::new();
    let (callable, _, authority) = publishing(&mut fixture, vec![binder(0, 0); 2]);
    let template = collapsed(&fixture);
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    validate(&template, &callable, &authority, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units * 2 - 1,
        ..DecodeLimits::default()
    });
    validate(&template, &callable, &authority, &mut shared).unwrap();
    assert!(validate(&template, &callable, &authority, &mut shared).is_err());
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(
            validate(
                &template,
                &callable,
                &authority,
                &mut BudgetMeter::new(limits)
            )
            .is_err()
        );
    }
}
