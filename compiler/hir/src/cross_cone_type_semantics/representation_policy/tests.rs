use scoop_wire::{decode_canonical, encode};

use super::*;

#[test]
fn c_layout_has_closed_semantic_alignment_and_stable_wire() {
    assert_eq!(
        encode(&NominalCLayoutPolicyV1::Ordinary).unwrap(),
        [0xa1, 0, 1]
    );
    let policy = NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A16,
            packed: HirCLayoutValue::A1,
        },
    };
    assert_eq!(
        encode(&policy).unwrap(),
        [0xa2, 0, 2, 1, 0xa2, 1, 0xa1, 0, 6, 2, 0xa1, 0, 2]
    );
    let alignments = [
        HirCLayoutValue::Natural,
        HirCLayoutValue::A1,
        HirCLayoutValue::A2,
        HirCLayoutValue::A4,
        HirCLayoutValue::A8,
        HirCLayoutValue::A16,
    ];
    for aligned in alignments {
        for packed in alignments {
            let policy = NominalCLayoutPolicyV1::CLayout {
                contract: HirCLayoutContract { aligned, packed },
            };
            assert_eq!(
                decode_canonical::<NominalCLayoutPolicyV1>(&encode(&policy).unwrap()).unwrap(),
                policy
            );
        }
    }
}

#[test]
fn intrinsic_family_keeps_integer_width_and_signedness_and_generic_family_distinct() {
    let fixed =
        NominalIntrinsicRepresentationV1::new(IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_32));
    assert_eq!(
        encode(&fixed).unwrap(),
        [0xa3, 0, 1, 1, 0xa1, 0, 2, 2, 0xa1, 0, 3]
    );
    let mut encodings = std::collections::BTreeSet::new();
    for family in IntegerKind::ALL
        .into_iter()
        .map(IntrinsicTypeKind::Integer)
        .chain([
            IntrinsicTypeKind::Unit,
            IntrinsicTypeKind::Char,
            IntrinsicTypeKind::Boolean,
            IntrinsicTypeKind::String,
            IntrinsicTypeKind::Array,
            IntrinsicTypeKind::MutableArray,
            IntrinsicTypeKind::Ptr,
            IntrinsicTypeKind::FunPtr,
        ])
    {
        let representation = NominalIntrinsicRepresentationV1::new(family);
        let bytes = encode(&representation).unwrap();
        assert!(encodings.insert(bytes.clone()));
        assert_eq!(
            decode_canonical::<NominalIntrinsicRepresentationV1>(&bytes)
                .unwrap()
                .family(),
            family
        );
    }
    assert_eq!(encodings.len(), 16);
}

#[test]
fn representation_reader_rejects_unknown_family_alignment_and_extra_fields() {
    for bytes in [
        &[0xa1, 0, 10][..],
        &[0xa2, 0, 4, 1, 0][..],
        &[0xa1, 0, 1][..],
        &[0xa3, 0, 1, 1, 0xa1, 0, 3, 2, 0xa1, 0, 1][..],
        &[0xa3, 0, 1, 1, 0xa1, 0, 1, 2, 0xa1, 0, 5][..],
    ] {
        assert!(decode_canonical::<NominalIntrinsicRepresentationV1>(bytes).is_err());
    }
    for bytes in [
        &[0xa1, 0, 3][..],
        &[0xa2, 0, 1, 1, 0][..],
        &[0xa2, 0, 2, 1, 0xa2, 1, 0xa1, 0, 7, 2, 0xa1, 0, 1][..],
    ] {
        assert!(decode_canonical::<NominalCLayoutPolicyV1>(bytes).is_err());
    }
}
