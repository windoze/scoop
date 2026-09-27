use scoop_identity::{Effect, GcEffect, LocalValueSelector, SignatureTypeKey};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;
mod captures;
use crate::cross_cone_interface::default_templates::expression_test_support::{
    Fixture, ResolutionError, Resolver,
};
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableOperatorRoleV1, CallableSafetyV1,
    CallableSourceEffectsV1, CanonicalBinderUseListV1, CanonicalBooleanV1,
    CanonicalExportGenericCallableBodiesV1, CanonicalTemplateLocalTableV1,
    DecodedCanonicalExportGenericCallableBodiesV1, DefaultCallableDeclarationV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultStatementKindV1, DefaultStatementV1,
    GenericCallableBodiesResolutionError, GenericCallableBodyTableError,
    GenericTemplatePredicatesV1, OptionalDefaultExpressionV1, TemplateLocalDefinitionV1,
    TemplateLocalRecordV1,
};

#[test]
fn generic_return_body_round_trips_without_a_default_parameter_owner() {
    let fixture = Fixture::new();
    let expected = body(&fixture, vec![fixture.local()]).unwrap();
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    assert_eq!(bytes[0], 0xaa);

    let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let actual = decoded.resolve(&mut fixture.resolver()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        actual.owner(),
        DefaultCallableDeclarationV1::GenericFunction(fixture.generic_function)
    );
    assert!(matches!(
        actual.statements()[0].kind(),
        DefaultStatementKindV1::Return(_)
    ));
}

#[test]
fn parameter_abi_order_is_independent_of_the_canonical_local_table_order() {
    let fixture = Fixture::new();
    let first = fixture.local();
    let second = LocalValueSelector::Parameter {
        declaration_index: 1,
    };
    let mut expected = body(&fixture, vec![first.clone()]).unwrap();
    expected.locals = CanonicalTemplateLocalTableV1::try_new(vec![
        local(&fixture, second.clone()),
        local(&fixture, first.clone()),
    ])
    .unwrap();
    expected.parameters = vec![second, first];

    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.parameters, [1, 0]);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
}

#[test]
fn binder_mapping_and_inferred_conditions_keep_both_binder_scopes() {
    let fixture = Fixture::new();
    let mut expected = body(&fixture, vec![fixture.local()]).unwrap();
    let owner_binder = SignatureTypeKey::Binder { depth: 1, index: 0 };
    let method_binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    expected.type_parameters = binders(vec![owner_binder.clone(), method_binder.clone()]);
    expected.predicates =
        GenericTemplatePredicatesV1::new(binders(vec![owner_binder]), binders(vec![method_binder]));

    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
}

#[test]
fn builder_rejects_missing_and_repeated_parameter_locals() {
    let fixture = Fixture::new();
    let missing = LocalValueSelector::Parameter {
        declaration_index: 1,
    };
    assert_eq!(
        body(&fixture, vec![missing.clone()]),
        Err(GenericCallableBodyBuildError::MissingParameter {
            index: 0,
            selector: missing,
        })
    );
    assert_eq!(
        body(&fixture, vec![fixture.local(), fixture.local()]),
        Err(GenericCallableBodyBuildError::DuplicateParameter {
            index: 1,
            selector: fixture.local(),
        })
    );
}

#[test]
fn reader_rejects_out_of_range_and_repeated_parameter_indices() {
    let fixture = Fixture::new();
    let expected = body(&fixture, vec![fixture.local()]).unwrap();
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let mut decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    decoded.parameters = vec![1];
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver()),
        Err(GenericCallableBodyResolutionError::Parameter {
            index: 0,
            source: TemplateLocalLookupError::IndexOutOfRange { index: 1, len: 1 },
        })
    ));

    let mut decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    decoded.parameters = vec![0, 0];
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver()),
        Err(GenericCallableBodyResolutionError::Record(
            GenericCallableBodyBuildError::DuplicateParameter { index: 1, .. }
        ))
    ));
}

#[test]
fn extern_implementation_cannot_supply_a_template_body() {
    let fixture = Fixture::new();
    let expected = body(&fixture, vec![fixture.local()]).unwrap();
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    for implementation in [
        CallableImplementationV1::SourceExternScoop,
        CallableImplementationV1::SourceExternC,
    ] {
        let mut decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
        decoded.effects = decode_canonical(&encode(&effects(implementation)).unwrap()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture.resolver()),
            Err(GenericCallableBodyResolutionError::Record(
                GenericCallableBodyBuildError::BodylessImplementation(actual)
            )) if actual == implementation
        ));
    }
}

#[test]
fn body_reader_preserves_a_missing_typed_owner_error() {
    let fixture = Fixture::new();
    let expected = body(&fixture, vec![fixture.local()]).unwrap();
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(GenericCallableBodyResolutionError::Owner(ResolutionError))
    ));
}

#[test]
fn body_decoder_requires_all_ten_fields() {
    let error = decode_canonical::<DecodedExportGenericCallableBodyV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 10,
            actual: 0
        }
    );
}

#[test]
fn body_table_sorts_typed_owners_and_round_trips() {
    let fixture = Fixture::new();
    let first = body(&fixture, vec![fixture.local()]).unwrap();
    let mut second = first.clone();
    second.owner = DefaultCallableDeclarationV1::Generated(fixture.generated);
    let table =
        CanonicalExportGenericCallableBodiesV1::try_new(vec![second.clone(), first.clone()])
            .unwrap();
    assert_eq!(table.records(), &[first.clone(), second.clone()]);
    assert_eq!(table.get(first.owner()), Some(&first));
    assert_eq!(table.get(second.owner()), Some(&second));

    let bytes = encode(&table.index_locals().unwrap()).unwrap();
    let decoded: DecodedCanonicalExportGenericCallableBodiesV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), table);
    assert_eq!(
        CanonicalExportGenericCallableBodiesV1::try_new(vec![first.clone(), first.clone()]),
        Err(GenericCallableBodyTableError {
            index: 1,
            owner: first.owner()
        })
    );
}

#[test]
fn body_table_reader_rejects_duplicate_and_noncanonical_owner_order() {
    let fixture = Fixture::new();
    let first = body(&fixture, vec![fixture.local()]).unwrap();
    let mut second = first.clone();
    second.owner = DefaultCallableDeclarationV1::Generated(fixture.generated);
    let first_bytes = encode(&first.index_locals().unwrap()).unwrap();
    let second_bytes = encode(&second.index_locals().unwrap()).unwrap();

    for leading in [&first_bytes, &second_bytes] {
        let bytes = [b"\x82".as_slice(), leading, &first_bytes].concat();
        let decoded: DecodedCanonicalExportGenericCallableBodiesV1 =
            decode_canonical(&bytes).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture.resolver()),
            Err(GenericCallableBodiesResolutionError::Table(GenericCallableBodyTableError {
                index: 1,
                owner,
            })) if owner == first.owner()
        ));
    }
}

fn body(
    fixture: &Fixture,
    parameters: Vec<LocalValueSelector>,
) -> Result<ExportGenericCallableBodyV1, GenericCallableBodyBuildError> {
    let value = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Local(fixture.local()),
        fixture.value_type(),
        fixture.origin(),
    )
    .unwrap();
    ExportGenericCallableBodyV1::try_new(
        DefaultCallableDeclarationV1::GenericFunction(fixture.generic_function),
        CanonicalTemplateLocalTableV1::try_new(vec![local(fixture, fixture.local())]).unwrap(),
        parameters,
        vec![
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::present(value)),
                fixture.origin(),
            )
            .unwrap(),
        ],
        fixture.value_type(),
        effects(CallableImplementationV1::Scoop),
        binders(vec![fixture.value_type()]),
        GenericTemplatePredicatesV1::new(binders(Vec::new()), binders(Vec::new())),
        fixture.origin(),
        Vec::new(),
    )
}

fn local(fixture: &Fixture, selector: LocalValueSelector) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        fixture.value_type(),
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(fixture.origin()),
    )
    .unwrap()
}

fn effects(implementation: CallableImplementationV1) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Unsafe,
        GcEffect::NoGc,
        implementation,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn binders(arguments: Vec<SignatureTypeKey>) -> CanonicalBinderUseListV1 {
    CanonicalBinderUseListV1::try_new(arguments).unwrap()
}
