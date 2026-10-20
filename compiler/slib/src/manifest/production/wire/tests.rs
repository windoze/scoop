use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn library_manifest_wire_round_trips_consumed_fields() {
    let bytes = library_manifest_bytes();
    let decoded = decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn manifest_reader_requires_the_current_one_field_shape() {
    for header in [0xa0, 0xa2, 0xad] {
        let mut bytes = library_manifest_bytes();
        bytes[0] = header;
        assert!(decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes).is_err());
    }
}

#[test]
fn manifest_reader_requires_the_odr_directory_field_even_when_empty() {
    let mut missing = library_manifest_bytes();
    missing.drain(1..3);
    assert!(decode_canonical::<DecodedSingleConeProductionManifestV1>(&missing).is_err());
    let mut wrong_field = library_manifest_bytes();
    wrong_field[1] = 12;
    let error =
        decode_canonical::<DecodedSingleConeProductionManifestV1>(&wrong_field).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::UnexpectedField {
            expected: 11,
            actual: 12
        }
    );
}

pub(crate) fn library_manifest_bytes() -> Vec<u8> {
    vec![0xa1, 11, 0x80]
}
