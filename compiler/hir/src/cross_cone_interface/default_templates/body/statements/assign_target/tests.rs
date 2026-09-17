use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{DefaultExpressionKindV1, DefaultFieldRefV1};

use super::super::super::expressions::test_support::Fixture;

#[test]
fn assignment_target_variants_have_fixed_tags_and_round_trip() {
    let fixture = Fixture::new();
    let cases = [
        DefaultAssignTargetV1::Local {
            local: fixture.local(),
        },
        DefaultAssignTargetV1::Global {
            property: fixture.property,
        },
        DefaultAssignTargetV1::Index {
            array: Box::new(unit(&fixture)),
            index: Box::new(unit(&fixture)),
        },
        DefaultAssignTargetV1::Field {
            receiver: Box::new(unit(&fixture)),
            field: DefaultFieldRefV1::Tuple {
                declaration_index: 2,
            },
        },
    ];

    for (expected_tag, expected) in (1_u8..=4).zip(cases) {
        let bytes = encode(&expected.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultAssignTargetV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Ok(expected)
        );
    }
}

#[test]
fn assignment_target_reports_nested_expression_index_location() {
    let fixture = Fixture::new();
    let target = DefaultAssignTargetV1::Index {
        array: Box::new(
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::Local(scoop_identity::LocalValueSelector::Parameter {
                    declaration_index: 1,
                }),
                fixture.value_type(),
                fixture.origin(),
            )
            .unwrap(),
        ),
        index: Box::new(unit(&fixture)),
    };

    assert!(matches!(
        target.index_locals(&mut fixture.locals()),
        Err(DefaultAssignTargetIndexError::Expression {
            target_tag: 3,
            field: 1,
            ..
        })
    ));
}

#[test]
fn assignment_target_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error = decode_canonical::<DecodedDefaultAssignTargetV1>(
        &[0xa1, 0x00, 0x05],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let error = decode_canonical::<DecodedDefaultAssignTargetV1>(
        &[0xa1, 0x00, 0x01],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type(),
        fixture.origin(),
    )
    .unwrap()
}
