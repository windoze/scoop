use scoop_wire::{DecodeLimits, ResourceKind, decode_canonical, encode};

use super::*;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

#[test]
fn storage_wire_has_explicit_closed_variants_and_no_array_size_duplicate() {
    let zst = ValueStorageLayoutV1::zero_sized(16).unwrap();
    assert_eq!(encode(&zst).unwrap(), b"\xa2\x00\x01\x01\x10");
    let value = ValueStorageLayoutV1::inline(8, 8, RefScan::References(vec![0])).unwrap();
    let expected = b"\xa4\x00\x02\x01\x08\x02\x08\x03\xa2\x00\x02\x01\x81\x00";
    assert_eq!(encode(&value).unwrap(), expected);
    assert_eq!(
        encode(&ArrayElementStorageV1::from_value(&value)).unwrap(),
        expected
    );
    for value in [zst, value] {
        let bytes = encode(&value).unwrap();
        let decoded =
            decode_canonical::<DecodedValueStorageLayoutV1>(&bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.validate(TARGET).unwrap(), value);
        let array =
            decode_canonical::<DecodedArrayElementStorageV1>(&bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(
            array.validate(TARGET).unwrap(),
            ArrayElementStorageV1::from_value(&value)
        );
    }
}

#[test]
fn wire_rejects_unknown_tags_extra_fields_and_zero_scan_stride() {
    for bytes in [
        b"\xa1\x00\x03".as_slice(),
        b"\xa3\x00\x01\x01\x01\x02\x00",
        b"\xa5\x00\x02\x01\x08\x02\x08\x03\xa1\x00\x01\x04\x08",
    ] {
        assert!(
            decode_canonical::<DecodedValueStorageLayoutV1>(bytes, DecodeLimits::default())
                .is_err()
        );
    }
    // Runtime scan v1 has Array (tag 4), never a fixed Repeat (tag 5).
    assert!(matches!(
        decode_canonical::<DecodedRefScanV1>(b"\xa1\x00\x05", DecodeLimits::default())
            .unwrap_err()
            .kind(),
        WireErrorKind::UnknownTag { tag: 5 }
    ));
    let array = b"\xa5\x00\x04\x01\x00\x02\x08\x03\x00\x04\xa2\x00\x02\x01\x81\x00";
    assert_eq!(
        decode_canonical::<DecodedRefScanV1>(array, DecodeLimits::default())
            .unwrap()
            .validate(),
        Err(crate::RefScanValidationError::ZeroArrayStride)
    );
}

#[test]
fn decoded_storage_rechecks_target_and_reference_extent() {
    for (bytes, expected) in [
        (
            b"\xa2\x00\x01\x01\x03".as_slice(),
            TypeInstanceShapeError::AlignmentNotPowerOfTwo(3),
        ),
        (
            b"\xa2\x00\x01\x01\x18\x20",
            TypeInstanceShapeError::ManagedAlignmentTooLarge {
                actual: 32,
                maximum: 16,
            },
        ),
        (
            b"\xa4\x00\x02\x01\x08\x02\x08\x03\xa2\x00\x02\x01\x81\x08",
            TypeInstanceShapeError::Scan(crate::RefScanValidationError::OutOfBounds {
                offset: 8,
                size: 8,
                extent: 8,
            }),
        ),
    ] {
        let decoded =
            decode_canonical::<DecodedValueStorageLayoutV1>(bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(decoded.validate(TARGET), Err(expected));
    }
    let large = ValueStorageLayoutV1::inline(1_u64 << 63, 8, RefScan::None).unwrap();
    assert!(
        decode_canonical::<DecodedValueStorageLayoutV1>(
            &encode(&large).unwrap(),
            DecodeLimits::default()
        )
        .unwrap()
        .validate(TARGET)
        .is_err()
    );
}

#[test]
fn scan_decoder_limits_depth_and_counts_before_allocating_or_reading_children() {
    let mut deep = Vec::new();
    for _ in 0..65 {
        deep.extend_from_slice(b"\xa5\x00\x04\x01\x00\x02\x08\x03\x08\x04");
    }
    deep.extend_from_slice(b"\xa2\x00\x02\x01\x81\x00");
    let limits = DecodeLimits {
        cbor_nesting: 1_024,
        ..DecodeLimits::default()
    };
    let error = decode_canonical::<DecodedRefScanV1>(&deep, limits).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticRecursion,
            limit: 64,
            observed: 65
        }
    ));
    // A million offsets are rejected from the array count alone; the absent
    // elements cannot turn the failure into a later UnexpectedEnd error.
    let error =
        decode_canonical::<DecodedRefScanV1>(b"\xa2\x00\x02\x01\x9a\x00\x10\x00\x00", limits)
            .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticTableEntries,
            limit: 1_048_576,
            observed: 1_048_577
        }
    ));
}

#[test]
fn scan_reader_preserves_wire_order_until_semantic_validation() {
    let bytes = b"\xa2\x00\x02\x01\x82\x08\x00";
    let decoded = decode_canonical::<DecodedRefScanV1>(bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.validate(),
        Err(crate::RefScanValidationError::UnorderedReferences)
    );
}
