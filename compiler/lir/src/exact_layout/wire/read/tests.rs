use super::*;
use crate::exact_layout::tests::unit;
use scoop_wire::{decode_canonical, encode};

#[test]
fn complete_record_decoder_rejects_noncanonical_map_and_unknown_closed_tags() {
    let expected = ExactLayoutExportV1::from(unit());
    let mut bytes = encode(&expected).unwrap();
    bytes[0] = 0xa6;
    assert!(decode_canonical::<DecodedExactLayoutExportV1>(&bytes).is_err());
    for (bytes, valid) in [
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 1], true),
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 2], false),
        (vec![0xa2, 0, 2, 1, 0xa1, 0, 5], false),
        (vec![0xa2, 0, 9, 1, 0xa1, 0, 1], false),
        (vec![0xa3, 0, 7, 1, 0xa1, 0, 1, 2, 0], false),
    ] {
        assert_eq!(decode_canonical::<RawValue>(&bytes).is_ok(), valid);
    }
}
