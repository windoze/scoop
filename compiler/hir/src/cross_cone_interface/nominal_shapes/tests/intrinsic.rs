use super::*;
use crate::{IntegerKind, IntrinsicTypeKind, IntrinsicTypeTarget};

mod contracts;

fn families() -> impl Iterator<Item = IntrinsicTypeKind> {
    IntegerKind::ALL
        .into_iter()
        .map(IntrinsicTypeKind::Integer)
        .chain([
            IntrinsicTypeKind::Boolean,
            IntrinsicTypeKind::String,
            IntrinsicTypeKind::Array,
            IntrinsicTypeKind::MutableArray,
            IntrinsicTypeKind::Ptr,
            IntrinsicTypeKind::FunPtr,
        ])
}

#[test]
fn intrinsic_source_shapes_keep_every_family_and_fixed_tag_through_both_readers() {
    let mut identities = authority(&fixture());

    let mut encodings = BTreeSet::new();
    for family in families() {
        let representation = NominalIntrinsicRepresentationV1::new(family);
        let shape = NominalSourceShapeV1::Intrinsic(representation);
        assert_eq!(
            shape.kind(),
            match family.target() {
                IntrinsicTypeTarget::Class => PublicNominalKindV1::Class,
                IntrinsicTypeTarget::Struct => PublicNominalKindV1::Struct,
            }
        );
        let bytes = encode(&shape).unwrap();
        assert_eq!(
            bytes,
            [&[0xa2, 0, 6, 1][..], &encode(&representation).unwrap()].concat()
        );
        assert!(encodings.insert(bytes.clone()));
        assert_eq!(encode(&decode_shape(&shape)).unwrap(), bytes);
        assert_eq!(
            decode_shape(&shape).resolve(&mut identities).unwrap(),
            shape
        );
        assert_eq!(
            decode_shape(&shape).resolve(&mut identities).unwrap(),
            shape
        );
    }
    assert_eq!(encodings.len(), 14);
}

#[test]
fn intrinsic_shape_rejects_missing_family_extra_fields_and_unknown_kinds() {
    for bytes in [
        &[0xa1, 0, 6][..],
        &[0xa3, 0, 6, 1, 0xa1, 0, 2, 2, 0][..],
        &[0xa2, 0, 6, 1, 0xa1, 0, 8][..],
        &[0xa2, 0, 6, 1, 0xa3, 0, 1, 1, 0xa1, 0, 3, 2, 0xa1, 0, 1][..],
        &[0xa2, 0, 6, 1, 0xa3, 0, 1, 1, 0xa1, 0, 1, 2, 0xa1, 0, 5][..],
    ] {
        assert!(decode_canonical::<DecodedNominalSourceShapeV1>(bytes).is_err());
    }
}
