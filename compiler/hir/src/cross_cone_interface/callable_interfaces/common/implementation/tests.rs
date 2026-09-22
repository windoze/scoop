use super::*;

mod effects;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn every_typed_intrinsic_kind_round_trips_as_a_complete_implementation() {
    for kind in crate::intrinsic_function_kinds() {
        let implementation = CallableImplementationV1::Intrinsic(kind);
        let bytes = encode(&implementation).unwrap();
        assert_eq!(&bytes[..4], &[0xa2, 0, 2, 1]);
        let decoded: CallableImplementationV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded, implementation);
    }
}

#[test]
fn non_intrinsic_implementations_have_no_kind_payload() {
    for (implementation, tag) in [
        (CallableImplementationV1::Scoop, 1),
        (CallableImplementationV1::SourceExternScoop, 3),
        (CallableImplementationV1::SourceExternC, 4),
    ] {
        let bytes = encode(&implementation).unwrap();
        assert_eq!(bytes, [0xa1, 0, tag]);
        assert_eq!(
            decode_canonical::<CallableImplementationV1>(&bytes, DecodeLimits::default()).unwrap(),
            implementation
        );
    }
}

#[test]
fn implementation_reader_rejects_old_leaf_tags_and_incomplete_or_open_sums() {
    for bytes in [
        vec![1],
        vec![2],
        vec![3],
        vec![4],
        vec![0xa0],
        vec![0xa1, 0, 2],
        vec![0xa2, 0, 2, 1, 0xa0],
        vec![0xa2, 0, 1, 1, 0xa1, 0, 5],
        vec![0xa1, 0, 5],
        vec![0xa3, 0, 2, 1, 0xa1, 0, 5, 2, 0],
    ] {
        assert!(
            decode_canonical::<CallableImplementationV1>(&bytes, DecodeLimits::default()).is_err(),
            "{bytes:02x?}"
        );
    }
}
