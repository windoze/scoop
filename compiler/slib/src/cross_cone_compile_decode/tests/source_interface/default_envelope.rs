use scoop_hir::{
    DefaultBodyProviderEnvelopeSemanticValidationError as BodyError,
    DefaultBodyProviderTypeSiteV1 as Site, DefaultTemplateSourceEnvelopeError as EnvelopeError,
    SignatureBinderScopeError as ScopeError, SignatureTypeSemanticError as TypeError,
    TemplateLocalScopeValidationError,
};
use scoop_identity::{LocalValueSelector, NonEmptyVec};

use super::*;
use crate::CrossConeHirDefaultProviderContractError as Error;

mod local_functions;
pub(super) mod support;
pub(super) use local_functions::fixture as local_function_fixture;
use support::{add_local, add_owner_expression, local_path};

#[test]
fn ordinary_default_envelope_accepts_unused_locals_in_the_provider_scope() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let ty = fixture.interface.default_templates().records()[0]
        .result()
        .clone();
    add_local(&mut fixture, local_path(0, 1), ty);
    validate_until_type_alias(&fixture.artifact())
        .validate_source_interfaces(vec![])
        .unwrap();
}

#[test]
fn ordinary_default_envelope_checks_the_types_of_unused_locals() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    add_local(
        &mut fixture,
        local_path(0, 1),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
    );
    let EnvelopeError::LocalType { index, error } = failure(&fixture) else {
        panic!("unused local type must be checked")
    };
    assert_eq!(index, 1);
    assert!(matches!(
        *error,
        TypeError::BinderScope(ScopeError::DepthOutOfRange {
            depth: 0,
            available_depths: 0
        })
    ));
}

#[test]
fn ordinary_default_envelope_rejects_locals_from_another_default_path() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let ty = fixture.interface.default_templates().records()[0]
        .result()
        .clone();
    let wrong_path = local_path(1, 0);
    add_local(&mut fixture, wrong_path.clone(), ty);
    let EnvelopeError::LocalScope(TemplateLocalScopeValidationError::LocalOutsideDefinitionPath {
        index,
        selector,
    }) = failure(&fixture)
    else {
        panic!("unused local path must belong to its default")
    };
    assert_eq!(index, 1);
    assert_eq!(
        selector,
        LocalValueSelector::LocalDeclaration { path: wrong_path }
    );
}

#[test]
fn ordinary_default_envelope_checks_nested_expression_owner_types() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    add_owner_expression(
        &mut fixture,
        SignatureTypeKey::Binder { depth: 0, index: 0 },
    );
    assert_body_scope(
        &fixture,
        Site::StructConstructOwner,
        ScopeError::DepthOutOfRange {
            depth: 0,
            available_depths: 0,
        },
    );
}

#[test]
fn ordinary_default_envelope_uses_actual_generic_nominal_arity() {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let origin = fixture
        .interface
        .nominal_interfaces()
        .all_records()
        .find_map(|record| match record.declaration() {
            SourceNominalId::GenericTemplate(id) => Some(id),
            SourceNominalId::Concrete(_) => None,
        })
        .unwrap();
    let ty = fixture.interface.default_templates().records()[0]
        .result()
        .clone();
    add_owner_expression(
        &mut fixture,
        SignatureTypeKey::NominalApplication {
            origin,
            arguments: NonEmptyVec::from_first(ty.clone(), [ty]),
        },
    );
    let (site, error) = body_failure(&fixture);
    assert_eq!(site, Site::StructConstructOwner);
    assert!(matches!(
        error,
        TypeError::GenericNominalArity {
            expected: 1,
            actual: 2,
            ..
        }
    ));
}

fn assert_body_scope(fixture: &CallableSourceSurface, expected_site: Site, expected: ScopeError) {
    let (site, error) = body_failure(fixture);
    assert_eq!(site, expected_site);
    assert!(matches!(error, TypeError::BinderScope(actual) if actual == expected));
}

fn body_failure(
    fixture: &CallableSourceSurface,
) -> (Site, TypeError<crate::DefaultMetadataNominalError>) {
    let EnvelopeError::Body(error) = failure(fixture) else {
        panic!("expected body envelope error")
    };
    let BodyError::Type {
        site,
        definition_origin,
        error,
    } = *error
    else {
        panic!("expected provider type error")
    };
    assert_eq!(
        *definition_origin,
        *fixture.interface.default_templates().records()[0].definition_origin()
    );
    (site, *error)
}

fn failure(fixture: &CallableSourceSurface) -> EnvelopeError<crate::DefaultMetadataNominalError> {
    let bytes = fixture.artifact();
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultProviderContract(Error::Template {
        index,
        key,
        source,
    })) = validate_until_type_alias(&bytes).validate_source_interfaces(vec![])
    else {
        panic!("invalid type envelope must fail the ordinary source-interface gate")
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), fixture.owner);
    assert_eq!(key.parameter_position(), 0);
    assert!(
        source.to_string().contains("default")
            || source.to_string().contains("provider-scoped type")
    );
    let Error::Envelope(error) = *source else {
        panic!("expected source envelope error")
    };
    *error
}
