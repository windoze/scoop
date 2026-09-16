use scoop_wire::{
    DecodeLimits, Encoder, ResourceKind, WireEncode, WireErrorKind, decode_canonical, encode,
};

use super::*;

#[test]
fn every_integer_variant_has_fixed_wire_and_roundtrips_raw_bits() {
    let cases = [
        (
            CanonicalIntegerConstantV1::Signed8(0xff),
            vec![0xa2, 0x00, 0x01, 0x01, 0x18, 0xff],
        ),
        (
            CanonicalIntegerConstantV1::Signed16(0xfffe),
            vec![0xa2, 0x00, 0x02, 0x01, 0x19, 0xff, 0xfe],
        ),
        (
            CanonicalIntegerConstantV1::Signed32(0xffff_fffd),
            vec![0xa2, 0x00, 0x03, 0x01, 0x1a, 0xff, 0xff, 0xff, 0xfd],
        ),
        (
            CanonicalIntegerConstantV1::Signed64(0xffff_ffff_ffff_fffc),
            vec![
                0xa2, 0x00, 0x04, 0x01, 0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc,
            ],
        ),
        (
            CanonicalIntegerConstantV1::Unsigned8(0xab),
            vec![0xa2, 0x00, 0x05, 0x01, 0x18, 0xab],
        ),
        (
            CanonicalIntegerConstantV1::Unsigned16(0xcdef),
            vec![0xa2, 0x00, 0x06, 0x01, 0x19, 0xcd, 0xef],
        ),
        (
            CanonicalIntegerConstantV1::Unsigned32(0x89ab_cdef),
            vec![0xa2, 0x00, 0x07, 0x01, 0x1a, 0x89, 0xab, 0xcd, 0xef],
        ),
        (
            CanonicalIntegerConstantV1::Unsigned64(0x0123_4567_89ab_cdef),
            vec![
                0xa2, 0x00, 0x08, 0x01, 0x1b, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
            ],
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(encode(&value).unwrap(), expected);
        assert_eq!(
            decode_canonical::<CanonicalIntegerConstantV1>(&expected, DecodeLimits::default())
                .unwrap(),
            value
        );
        assert_eq!(value.kind(), HirIntegerConstant::from(value).kind());
        assert_eq!(value.raw_bits(), HirIntegerConstant::from(value).raw_bits());
    }
}

#[test]
fn integer_decoder_rejects_every_narrow_payload_overflow() {
    let cases = [
        (1, u64::from(u8::MAX) + 1),
        (2, u64::from(u16::MAX) + 1),
        (3, u64::from(u32::MAX) + 1),
        (5, u64::from(u8::MAX) + 1),
        (6, u64::from(u16::MAX) + 1),
        (7, u64::from(u32::MAX) + 1),
    ];

    for (tag, raw_bits) in cases {
        let encoded = encode(&UncheckedInteger { tag, raw_bits }).unwrap();
        let error =
            decode_canonical::<CanonicalIntegerConstantV1>(&encoded, DecodeLimits::default())
                .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
        assert_eq!(error.path().to_string(), "$.1");
    }
}

#[test]
fn integer_decoder_rejects_unknown_tags_and_wrong_sum_shape() {
    let unknown = decode_canonical::<CanonicalIntegerConstantV1>(
        &[0xa2, 0x00, 0x09, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let wrong_shape = decode_canonical::<CanonicalIntegerConstantV1>(
        &[0xa1, 0x00, 0x01],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn boolean_wire_is_an_explicit_unsigned_enumeration() {
    assert_eq!(encode(&CanonicalBooleanV1::False).unwrap(), vec![0x01]);
    assert_eq!(encode(&CanonicalBooleanV1::True).unwrap(), vec![0x02]);
    assert!(!CanonicalBooleanV1::False.value());
    assert!(CanonicalBooleanV1::True.value());

    let native_boolean =
        decode_canonical::<CanonicalBooleanV1>(&[0xf5], DecodeLimits::default()).unwrap_err();
    assert_eq!(native_boolean.kind(), &WireErrorKind::UnexpectedEnd);

    let unknown =
        decode_canonical::<CanonicalBooleanV1>(&[0x03], DecodeLimits::default()).unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn const_value_variants_have_fixed_wire_and_roundtrip() {
    let cases = [
        (
            CanonicalConstValueV1::Integer(CanonicalIntegerConstantV1::Signed8(0xff)),
            vec![0xa2, 0x00, 0x01, 0x01, 0xa2, 0x00, 0x01, 0x01, 0x18, 0xff],
        ),
        (
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            vec![0xa2, 0x00, 0x02, 0x01, 0x02],
        ),
        (
            CanonicalConstValueV1::String("A\0é".to_owned()),
            vec![0xa2, 0x00, 0x03, 0x01, 0x64, 0x41, 0x00, 0xc3, 0xa9],
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(encode(&value).unwrap(), expected);
        assert_eq!(
            decode_canonical::<CanonicalConstValueV1>(&expected, DecodeLimits::default()).unwrap(),
            value
        );
    }
}

#[test]
fn const_value_decoder_rejects_unknown_tags_wrong_shape_and_native_boolean() {
    let unknown = decode_canonical::<CanonicalConstValueV1>(
        &[0xa2, 0x00, 0x04, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 4 });

    let wrong_shape =
        decode_canonical::<CanonicalConstValueV1>(&[0xa1, 0x00, 0x01], DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );

    let native_boolean = decode_canonical::<CanonicalConstValueV1>(
        &[0xa2, 0x00, 0x02, 0x01, 0xf5],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(native_boolean.kind(), &WireErrorKind::UnexpectedEnd);
    assert_eq!(native_boolean.path().to_string(), "$.1");
}

#[test]
fn string_values_preserve_utf8_identity_and_obey_both_byte_budgets() {
    let composed = CanonicalConstValueV1::String("é\0".to_owned());
    let decomposed = CanonicalConstValueV1::String("e\u{301}\0".to_owned());
    assert_ne!(composed, decomposed);
    for value in [&composed, &decomposed] {
        let encoded = encode(value).unwrap();
        assert_eq!(
            decode_canonical::<CanonicalConstValueV1>(&encoded, DecodeLimits::default()).unwrap(),
            *value
        );
    }

    let encoded = encode(&CanonicalConstValueV1::String("four".to_owned())).unwrap();
    let semantic_error = decode_canonical::<CanonicalConstValueV1>(
        &encoded,
        DecodeLimits {
            semantic_leaf_bytes: 3,
            ..DecodeLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        semantic_error.kind(),
        &WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticLeafBytes,
            limit: 3,
            observed: 4,
        }
    );
    assert_eq!(semantic_error.path().to_string(), "$.1");

    let owned_error = decode_canonical::<CanonicalConstValueV1>(
        &encoded,
        DecodeLimits {
            owned_bytes: 3,
            ..DecodeLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        owned_error.kind(),
        &WireErrorKind::LimitExceeded {
            resource: ResourceKind::OwnedBytes,
            limit: 3,
            observed: 4,
        }
    );
    assert_eq!(owned_error.path().to_string(), "$.1");
}

#[test]
fn const_values_convert_losslessly_to_and_from_local_hir() {
    let values = [
        ConstPropertyValue::Integer(HirIntegerConstant::Signed32(0xffff_ffd6)),
        ConstPropertyValue::Integer(HirIntegerConstant::Unsigned64(u64::MAX)),
        ConstPropertyValue::Boolean(false),
        ConstPropertyValue::String("constant\0value".to_owned()),
    ];

    for value in values {
        let canonical = CanonicalConstValueV1::from(value.clone());
        assert_eq!(ConstPropertyValue::from(canonical), value);
    }
}

struct UncheckedInteger {
    tag: u64,
    raw_bits: u64,
}

impl WireEncode for UncheckedInteger {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(self.tag)?;
        encoder.field(1)?;
        encoder.unsigned(self.raw_bits)
    }
}
