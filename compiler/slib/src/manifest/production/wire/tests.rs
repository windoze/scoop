use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn library_manifest_wire_round_trips_without_promoting_carried_values() {
    let bytes = library_manifest_bytes();
    let decoded =
        decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert!(ensure_projection_equality(&bytes, &bytes).is_ok());

    let mut changed = bytes.clone();
    changed[8] ^= 1;
    assert!(matches!(
        ensure_projection_equality(&bytes, &changed),
        Err(SingleConeProductionManifestValidationError::ProjectionMismatch)
    ));
}

#[test]
fn manifest_reader_rejects_old_extended_and_unknown_sum_shapes() {
    for bytes in [vec![0xa9], vec![0xab]] {
        assert!(
            decode_canonical::<DecodedSingleConeProductionManifestV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    assert!(
        decode_canonical::<DecodedArtifactDistributionClassV1>(
            &[0xa1, 0x00, 0x03],
            DecodeLimits::default()
        )
        .is_err()
    );
    assert!(
        decode_canonical::<DecodedSingleConeProductionOutputV1>(
            &[0xa2, 0x00, 0x01, 0x01, 0x00],
            DecodeLimits::default()
        )
        .is_err()
    );
}

pub(crate) fn library_manifest_bytes() -> Vec<u8> {
    let mut bytes = vec![0xaa];
    field(&mut bytes, 1);
    bytes.extend_from_slice(&[0xa1, 0x00, 0x01]);
    field(&mut bytes, 2);
    bytes.extend_from_slice(&[0xa1, 0x00, 0x01]);
    field(&mut bytes, 3);
    fixed(&mut bytes, 3);
    field(&mut bytes, 4);
    six_empty_tables(&mut bytes);
    field(&mut bytes, 5);
    six_empty_tables(&mut bytes);
    field(&mut bytes, 6);
    fixed(&mut bytes, 6);
    field(&mut bytes, 7);
    fixed(&mut bytes, 7);
    field(&mut bytes, 8);
    bytes.push(0x80);
    field(&mut bytes, 9);
    bytes.push(0x80);
    field(&mut bytes, 10);
    bytes.extend_from_slice(&[0xa1, 0x00, 0x01]);
    bytes
}

fn six_empty_tables(bytes: &mut Vec<u8>) {
    bytes.push(0xa6);
    for field_number in 1..=6 {
        field(bytes, field_number);
        bytes.push(0x80);
    }
}

fn fixed(bytes: &mut Vec<u8>, value: u8) {
    bytes.extend_from_slice(&[0x58, 0x20]);
    bytes.extend_from_slice(&[value; 32]);
}

fn field(bytes: &mut Vec<u8>, field: u8) {
    bytes.push(field);
}
