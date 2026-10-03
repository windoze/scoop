use super::*;
use crate::cross_cone_interface::default_templates::expression_test_support::definition_path;
use crate::{DefaultCallableBodyTypeArgumentsV1, DefaultCaptureV1, DefaultLambdaV1};

#[test]
fn closure_body_capture_read_round_trips_with_its_declared_input_type() {
    let fixture = Fixture::new();
    let expected = capturing_body(&fixture, DefaultExpressionKindV1::Capture(0));
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
}

#[test]
fn reader_rejects_capture_reads_when_the_body_input_is_absent() {
    let fixture = Fixture::new();
    let expected = capturing_body(&fixture, DefaultExpressionKindV1::Capture(0));
    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    let mut decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
    decoded.capture_types.clear();
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver()),
        Err(GenericCallableBodyResolutionError::Record(
            GenericCallableBodyBuildError::CaptureIndex { index: 0, count: 0 }
        ))
    ));
}

#[test]
fn nested_closure_capture_source_is_checked_in_the_enclosing_body() {
    let fixture = Fixture::new();
    let function_type = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: Vec::new(),
        result: Box::new(fixture.value_type()),
    };
    for index in [0, 1] {
        let lambda = DefaultLambdaV1::try_new(
            fixture.generated,
            definition_path(),
            function_type.clone(),
            DefaultCallableBodyTypeArgumentsV1::lexical(),
            vec![DefaultCaptureV1::from_enclosing_capture(
                index,
                fixture.value_type(),
                fixture.origin(),
            )],
            1,
        )
        .unwrap();
        let expected = capturing_body(&fixture, DefaultExpressionKindV1::Lambda(lambda));
        let bytes = encode(&expected.index_locals().unwrap()).unwrap();
        let decoded: DecodedExportGenericCallableBodyV1 = decode_canonical(&bytes).unwrap();
        if index == 0 {
            assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
        } else {
            assert!(matches!(
                decoded.resolve(&mut fixture.resolver()),
                Err(GenericCallableBodyResolutionError::Record(
                    GenericCallableBodyBuildError::CaptureIndex { index: 1, count: 1 }
                ))
            ));
        }
    }
}

fn capturing_body(fixture: &Fixture, kind: DefaultExpressionKindV1) -> ExportGenericCallableBodyV1 {
    let mut body = body(fixture, Vec::new()).unwrap();
    body.owner = DefaultCallableDeclarationV1::Generated(fixture.generated);
    body.capture_types = vec![fixture.value_type()];
    let value_type = match &kind {
        DefaultExpressionKindV1::Lambda(lambda) => lambda.function_type().clone(),
        _ => fixture.value_type(),
    };
    body.result = value_type.clone();
    let value = DefaultExpressionV1::try_new(
        kind,
        value_type,
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap();
    body.statements = vec![
        DefaultStatementV1::try_new(
            DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::present(value)),
            fixture.origin(),
        )
        .unwrap(),
    ];
    body
}
