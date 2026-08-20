use super::*;
use crate::SourceCallReceiver;

#[test]
fn default_call_receiver_is_preserved_by_the_shared_expression_codec() {
    let fixture = Fixture::new();
    for receiver in [
        SourceCallReceiver::NoReceiver,
        SourceCallReceiver::Receiver {
            static_type: fixture.value_type(),
        },
    ] {
        let value = expression(
            DefaultExpressionKindV1::Call {
                callee: fixture.callable(),
                arguments: vec![unit(&fixture)],
                receiver,
            },
            &fixture,
        );
        let bytes = encode(&value.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(expression_tag(&bytes), 57);
        let decoded: DecodedDefaultExpressionV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let mut missing = bytes.clone();
        assert_eq!(missing[2], 0xa4);
        missing[2] = 0xa3;
        assert!(matches!(
            decode_canonical::<DecodedDefaultExpressionV1>(&missing)
                .unwrap_err()
                .kind(),
            WireErrorKind::InvalidLength {
                expected: 4,
                actual: 3
            }
        ));
        let mut retired = bytes;
        assert_eq!(retired[5], 57);
        retired[5] = 44;
        assert_eq!(
            decode_canonical::<DecodedDefaultExpressionV1>(&retired)
                .unwrap_err()
                .kind(),
            &WireErrorKind::UnknownTag { tag: 44 }
        );
    }
}

#[test]
fn default_call_receiver_cannot_exist_without_a_logical_argument() {
    let fixture = Fixture::new();
    let unit = unit(&fixture);
    assert_eq!(
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Call {
                callee: fixture.callable(),
                arguments: vec![],
                receiver: SourceCallReceiver::Receiver {
                    static_type: fixture.value_type()
                },
            },
            unit.result_type().clone(),
            unit.definition_origin().clone().clone(),
            scoop_identity::EvaluationOrigin::at_definition(
                (unit.definition_origin().clone()).origin()
            ),
        ),
        Err(DefaultExpressionBuildError::MissingReceiverArgument)
    );
}
