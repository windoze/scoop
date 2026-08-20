use scoop_identity::{LexicalCallableRole, StructuralDefinitionSiteRole};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::super::test_support::*;
use super::*;

#[test]
fn lambda_round_trips_provider_body_and_capture_abi() {
    let fixture = Fixture::new();
    let expected = DefaultLambdaV1::try_new(
        fixture.lambda_body,
        path(StructuralDefinitionSiteRole::Lambda, 0),
        function_type(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        vec![fixture.capture(0)],
        2,
    )
    .unwrap();
    let mut locals = LocalResolver::new(vec![parameter(0)]);
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();

    assert_eq!(bytes[0], 0xa6);
    let decoded: DecodedDefaultLambdaV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected.clone())
    );
    assert_eq!(expected.body(), fixture.lambda_body);
    assert!(expected.body_type_arguments().is_lexical());
    assert_eq!(expected.capture_count(), 1);
    assert_eq!(expected.owner_type_parameter_count(), 2);
}

#[test]
fn anonymous_function_round_trips_explicit_body_arguments() {
    let fixture = Fixture::new();
    let body_arguments = DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![binder(2)]).unwrap();
    let expected = DefaultAnonymousFunctionV1::try_new(
        fixture.anonymous_body,
        path(StructuralDefinitionSiteRole::Lambda, 1),
        function_type(),
        body_arguments,
        vec![fixture.capture(0)],
        3,
    )
    .unwrap();
    let mut locals = LocalResolver::new(vec![parameter(0)]);
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultAnonymousFunctionV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected.clone())
    );
    assert_eq!(expected.body(), fixture.anonymous_body);
    assert_eq!(
        expected.body_type_arguments().explicit_arguments(),
        Some([binder(2)].as_slice())
    );
}

#[test]
fn lexical_callable_reports_missing_body_identity() {
    let fixture = Fixture::new();
    let missing = lexical_body(fixture.function, LexicalCallableRole::LambdaBody, 9);
    let lambda = DefaultLambdaV1::try_new(
        missing,
        path(StructuralDefinitionSiteRole::Lambda, 9),
        function_type(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        Vec::new(),
        0,
    )
    .unwrap();
    let mut locals = LocalResolver::new(Vec::new());
    let decoded: DecodedDefaultLambdaV1 =
        decode_canonical(&encode(&lambda.index_locals(&mut locals).unwrap()).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Err(DefaultLexicalCallableResolutionError::Body(ResolutionError))
    );
}

#[test]
fn lexical_callable_index_error_identifies_capture_position() {
    let fixture = Fixture::new();
    let lambda = DefaultLambdaV1::try_new(
        fixture.lambda_body,
        path(StructuralDefinitionSiteRole::Lambda, 0),
        function_type(),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        vec![fixture.capture(0), fixture.capture(2)],
        0,
    )
    .unwrap();
    let mut locals = LocalResolver::new(vec![parameter(0)]);

    assert_eq!(
        lambda.index_locals(&mut locals).unwrap_err(),
        DefaultLexicalCallableIndexError::Capture {
            index: 1,
            error: DefaultCaptureIndexError::Source(LocalError::MissingSelector(parameter(2))),
        }
    );
}

#[test]
fn lexical_callable_decoders_require_all_six_fields() {
    for error in [
        decode_canonical::<DecodedDefaultLambdaV1>(&[0xa0]).unwrap_err(),
        decode_canonical::<DecodedDefaultAnonymousFunctionV1>(&[0xa0]).unwrap_err(),
    ] {
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 6,
                actual: 0,
            }
        );
    }
}
