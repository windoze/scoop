use super::*;
use crate::{CanonicalBooleanV1, HirFloatConstant, IntegerKind};
use scoop_wire::{decode_canonical, encode};

#[test]
fn annotation_array_wire_retains_empty_types_order_and_float_bits() {
    let empty = CanonicalAnnotationValueV1::Array {
        element_type: CanonicalConstValueKindV1::String,
        elements: vec![],
    };
    assert_eq!(
        encode(&empty).unwrap(),
        [0xa3, 0, 2, 1, 0xa1, 0, 3, 2, 0x80]
    );
    let mut values = vec![
        empty,
        CanonicalAnnotationValueV1::Scalar(CanonicalConstValueV1::Boolean(
            CanonicalBooleanV1::True,
        )),
    ];
    values.push(CanonicalAnnotationValueV1::Array {
        element_type: CanonicalConstValueKindV1::Float(FloatKind::F32),
        elements: [0x8000_0000, 0x7fc0_0123, 1, 0x8000_0000]
            .into_iter()
            .map(|bits| CanonicalConstValueV1::Float(HirFloatConstant::F32(bits)))
            .collect(),
    });
    values.extend(
        IntegerKind::ALL
            .into_iter()
            .map(|kind| CanonicalAnnotationValueV1::Array {
                element_type: CanonicalConstValueKindV1::Integer(kind),
                elements: vec![],
            }),
    );
    values.extend(
        [
            CanonicalConstValueKindV1::Char,
            CanonicalConstValueKindV1::Boolean,
            CanonicalConstValueKindV1::Float(FloatKind::F64),
        ]
        .into_iter()
        .map(|kind| CanonicalAnnotationValueV1::Array {
            element_type: kind,
            elements: vec![],
        }),
    );
    for value in values {
        let bytes = encode(&value).unwrap();
        assert_eq!(
            decode_canonical::<CanonicalAnnotationValueV1>(&bytes).unwrap(),
            value
        );
    }
}

#[test]
fn annotation_value_decoder_rejects_unknown_tags_and_missing_array_type() {
    let error = decode_canonical::<CanonicalAnnotationValueV1>(&[0xa1, 0, 3]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
    let error = decode_canonical::<CanonicalAnnotationValueV1>(&[0xa2, 0, 2, 2, 0x80]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 2
        }
    );
    let error =
        decode_canonical::<CanonicalAnnotationValueV1>(&[0xa3, 0, 2, 1, 0xa1, 0, 6, 2, 0x80])
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 6 });
}
