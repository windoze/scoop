use super::*;

mod effects;
use scoop_wire::{decode_canonical, encode};

#[test]
fn every_typed_intrinsic_kind_round_trips_as_a_complete_implementation() {
    for kind in crate::intrinsic_function_kinds() {
        let implementation = CallableImplementationV1::Intrinsic(kind);
        let bytes = encode(&implementation).unwrap();
        assert_eq!(&bytes[..4], &[0xa2, 0, 2, 1]);
        let decoded: CallableImplementationV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, implementation);
    }
}

#[test]
fn ordinary_scoop_implementations_have_no_kind_payload() {
    for (implementation, tag) in [
        (CallableImplementationV1::Scoop, 1),
        (CallableImplementationV1::SourceExternScoop, 3),
    ] {
        let bytes = encode(&implementation).unwrap();
        assert_eq!(bytes, [0xa1, 0, tag]);
        assert_eq!(
            decode_canonical::<CallableImplementationV1>(&bytes).unwrap(),
            implementation
        );
    }
}

#[test]
fn c_implementations_preserve_their_required_caller_mode() {
    for (mode, tag) in [(CAbiCallMode::NativeSafe, 1), (CAbiCallMode::GcLeaf, 2)] {
        let implementation = CallableImplementationV1::SourceExternC(mode);
        let bytes = encode(&implementation).unwrap();
        assert_eq!(bytes, [0xa2, 0, 4, 1, tag]);
        assert_eq!(
            decode_canonical::<CallableImplementationV1>(&bytes).unwrap(),
            implementation
        );
    }
    for bytes in [
        vec![0xa1, 0, 4],
        vec![0xa2, 0, 4, 1, 0],
        vec![0xa2, 0, 4, 1, 3],
    ] {
        assert!(decode_canonical::<CallableImplementationV1>(&bytes).is_err());
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
            decode_canonical::<CallableImplementationV1>(&bytes).is_err(),
            "{bytes:02x?}"
        );
    }
}
