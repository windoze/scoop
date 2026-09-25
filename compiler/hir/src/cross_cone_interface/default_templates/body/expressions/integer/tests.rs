use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn integer_operation_variants_have_fixed_tags_and_round_trip() {
    let cases = [
        DefaultIntegerOperationV1::NoGc {
            kind: DefaultIntegerKindV1::Signed32,
            operation: DefaultNoGcIntegerOperationV1::Add,
        },
        DefaultIntegerOperationV1::Managed {
            kind: DefaultIntegerKindV1::Unsigned64,
            operation: DefaultIntegerDivRemV1::Rem,
        },
    ];
    for (expected_tag, expected) in [1, 2].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[0], 0xa3);
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultIntegerOperationV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, expected);
    }
}

#[test]
fn integer_operation_decoder_rejects_legacy_callee_field() {
    for tag in [1, 2] {
        let error = decode_canonical::<DecodedDefaultIntegerOperationV1>(&[
            0xa4, 0x00, tag, 0x01, 0x03, 0x02, 0x01, 0x03, 0xa0,
        ])
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 3,
                actual: 4
            }
        );
    }
}

#[test]
fn integer_operation_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error = decode_canonical::<DecodedDefaultIntegerOperationV1>(&[
        0xa3, 0x00, 0x03, 0x01, 0x03, 0x02, 0x01,
    ])
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
    let error =
        decode_canonical::<DecodedDefaultIntegerOperationV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 1
        }
    );
}

#[test]
fn integer_conversion_decoder_rejects_legacy_callee_field() {
    let error = decode_canonical::<crate::DecodedDefaultExpressionV1>(&[
        0xa3, 0x01, 0xa5, 0x00, 0x18, 50, 0, 0, 0, 0, 0, 0, 0, 0,
    ])
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 5
        }
    );
}

#[test]
fn integer_literal_equality_decoder_rejects_legacy_callee_field() {
    let error = decode_canonical::<crate::DecodedDefaultLiteralEqualityV1>(&[
        0xa3, 0x00, 0x01, 0x01, 0x03, 0x02, 0xa0,
    ])
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 3
        }
    );
}
