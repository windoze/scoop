use scoop_wire::{BudgetMeter, DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;

fn usage(index: u32, receiver: ProtectedDefaultReceiverUseV1) -> ProtectedDefaultExpressionUseV1 {
    ProtectedDefaultExpressionUseV1::new(index, receiver)
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn expression_use_wire_preserves_all_four_receiver_forms_and_u32_indices() {
    let cases = [
        (ProtectedDefaultReceiverUseV1::None, vec![0xa1, 0, 1]),
        (
            ProtectedDefaultReceiverUseV1::ImplicitThis,
            vec![0xa1, 0, 2],
        ),
        (
            ProtectedDefaultReceiverUseV1::Explicit {
                receiver_expression_index: u32::MAX,
            },
            vec![0xa2, 0, 3, 1, 0x1a, 0xff, 0xff, 0xff, 0xff],
        ),
        (
            ProtectedDefaultReceiverUseV1::ConstructorDelegation,
            vec![0xa1, 0, 4],
        ),
    ];
    for (receiver, expected) in cases {
        assert_eq!(encode(&receiver).unwrap(), expected);
        assert_eq!(
            decode_canonical::<ProtectedDefaultReceiverUseV1>(&expected, DecodeLimits::default())
                .unwrap(),
            receiver
        );
        let value = usage(u32::MAX, receiver);
        let bytes = encode(&value).unwrap();
        assert_eq!(&bytes[..3], &[0xa2, 1, 0x1a]);
        assert_eq!(
            decode_canonical::<ProtectedDefaultExpressionUseV1>(&bytes, DecodeLimits::default())
                .unwrap(),
            value
        );
        assert_eq!(value.expression_index(), u32::MAX);
        assert_eq!(value.receiver_use(), receiver);
    }
}

#[test]
fn canonical_uses_sort_by_expression_then_receiver_tag_and_index() {
    let ordered = vec![
        usage(0, ProtectedDefaultReceiverUseV1::ConstructorDelegation),
        usage(1, ProtectedDefaultReceiverUseV1::None),
        usage(1, ProtectedDefaultReceiverUseV1::ImplicitThis),
        usage(
            1,
            ProtectedDefaultReceiverUseV1::Explicit {
                receiver_expression_index: 0,
            },
        ),
        usage(
            1,
            ProtectedDefaultReceiverUseV1::Explicit {
                receiver_expression_index: 10,
            },
        ),
        usage(1, ProtectedDefaultReceiverUseV1::ConstructorDelegation),
    ];
    let values =
        CanonicalProtectedDefaultExpressionUsesV1::try_new(ordered.iter().rev().copied().collect())
            .unwrap();
    assert_eq!(values.values(), ordered);
    let bytes = encode(&values).unwrap();
    let decoded: DecodedCanonicalProtectedDefaultExpressionUsesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut meter()).unwrap(), values);
}

#[test]
fn producer_and_reader_reject_duplicate_uses_and_reader_rejects_reverse_order() {
    let first = usage(0, ProtectedDefaultReceiverUseV1::None);
    let second = usage(1, ProtectedDefaultReceiverUseV1::None);
    assert_eq!(
        CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![first, first]),
        Err(ProtectedDefaultExpressionUsesBuildError::Duplicate { index: 1 })
    );
    for (values, expected) in [
        (
            vec![first, first],
            ProtectedDefaultExpressionUsesBuildError::Duplicate { index: 1 },
        ),
        (
            vec![second, first],
            ProtectedDefaultExpressionUsesBuildError::NonCanonicalOrder { index: 1 },
        ),
    ] {
        let decoded: DecodedCanonicalProtectedDefaultExpressionUsesV1 =
            decode_canonical(&encode(&RawUses(values)).unwrap(), DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded.resolve(&mut meter()),
            Err(ProtectedDefaultExpressionUsesResolutionError::Build(
                expected
            ))
        );
    }
}

#[test]
fn receiver_and_use_readers_reject_unknown_tags_wrong_fields_and_wide_indices() {
    for bytes in [
        vec![0xa1, 0, 5],
        vec![0xa1, 0, 3],
        vec![0xa2, 0, 1, 1, 0],
        vec![0xa2, 0, 3, 1, 0x1b, 0, 0, 0, 1, 0, 0, 0, 0],
    ] {
        assert!(
            decode_canonical::<ProtectedDefaultReceiverUseV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
    for bytes in [
        vec![0xa1, 1, 0],
        vec![0xa2, 1, 0x1b, 0, 0, 0, 1, 0, 0, 0, 0, 2, 0xa1, 0, 1],
    ] {
        assert!(
            decode_canonical::<ProtectedDefaultExpressionUseV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn canonical_use_validation_spends_the_shared_meter_and_empty_data_remains_explicit() {
    let values = CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![usage(
        2,
        ProtectedDefaultReceiverUseV1::ImplicitThis,
    )])
    .unwrap();
    let decoded: DecodedCanonicalProtectedDefaultExpressionUsesV1 =
        decode_canonical(&encode(&values).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut BudgetMeter::new(DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        })),
        Err(ProtectedDefaultExpressionUsesResolutionError::Resource(_))
    ));
    let empty = CanonicalProtectedDefaultExpressionUsesV1::try_new(Vec::new()).unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    let decoded: DecodedCanonicalProtectedDefaultExpressionUsesV1 =
        decode_canonical(&[0x80], DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut meter()).unwrap(), empty);
}

struct RawUses(Vec<ProtectedDefaultExpressionUseV1>);
impl WireEncode for RawUses {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.0)
    }
}
