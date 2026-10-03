use scoop_hir::{
    DefaultTemplateDeclarationContractError as ContractError,
    TemplateReceiverSemanticValidationError, TemplateValueParameterSemanticValidationError,
};

use super::*;
use crate::CrossConeHirDefaultProviderContractError as Error;

mod support;
use support::{Change, fixture};

#[test]
fn ordinary_reader_rejects_a_default_without_its_provider_ordinal() {
    assert!(matches!(failure(Change::Ordinal), Error::DefinitionPath));
}

#[test]
fn ordinary_reader_rejects_extra_provider_binders_even_when_unused() {
    assert!(matches!(
        contract_failure(Change::Mapping),
        ContractError::MappingArity
    ));
}

#[test]
fn ordinary_reader_checks_the_original_default_result_type() {
    assert!(matches!(
        contract_failure(Change::Result),
        ContractError::ResultType
    ));
}

#[test]
fn ordinary_reader_rejects_a_receiver_on_a_top_level_default() {
    assert!(matches!(
        contract_failure(Change::Receiver),
        ContractError::Receiver(TemplateReceiverSemanticValidationError::Unexpected { .. },)
    ));
}

#[test]
fn ordinary_reader_rejects_a_parameter_prefix_before_position_zero() {
    assert!(matches!(
        contract_failure(Change::Prefix),
        ContractError::Prefix(TemplateValueParameterSemanticValidationError::PrefixArity {
            expected: 0,
            actual: 1
        },)
    ));
}

#[test]
fn ordinary_reader_checks_default_suspend_permission_against_its_declaration() {
    assert!(matches!(
        contract_failure(Change::Suspend),
        ContractError::SuspendPermission
    ));
}

fn contract_failure(change: Change) -> ContractError<crate::DefaultMetadataNominalError> {
    let Error::Contract(error) = failure(change) else {
        panic!("source declaration contract must reject the forged template")
    };
    *error
}

fn failure(change: Change) -> Error {
    let fixture = fixture(change);
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract(Error::Template {
        index,
        key,
        source,
    })) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("invalid provider contract must not pass the ordinary source-interface gate")
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), fixture.owner);
    assert_eq!(key.parameter_position(), 0);
    let expected = match change {
        Change::Ordinal => "default path does not identify a provider default parameter".to_owned(),
        Change::Mapping => {
            "default source mapping differs from its provider binder count".to_owned()
        }
        Change::Result => {
            "default source result differs from its original provider parameter".to_owned()
        }
        Change::Receiver => format!(
            "default template has unexpected receiver of type {:?}",
            fixture.interface.default_templates().records()[0].result()
        ),
        Change::Prefix => {
            "default template has 1 preceding value parameters, expected 0".to_owned()
        }
        Change::Suspend => {
            "default source suspend permission differs from its declarations".to_owned()
        }
    };
    assert_eq!(source.to_string(), expected);
    *source
}
