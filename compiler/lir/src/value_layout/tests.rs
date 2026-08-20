use super::*;
use crate::RefScanValidationError;

#[test]
fn storage_constructors_close_alignment_size_and_scan_invariants() {
    for alignment in [0, 3, 6] {
        assert!(ValueStorageLayoutV1::zero_sized(alignment).is_err());
    }
    assert_eq!(
        ValueStorageLayoutV1::inline(0, 8, RefScan::None),
        Err(TypeInstanceShapeError::ZeroInlineSize)
    );
    assert_eq!(
        ValueStorageLayoutV1::inline(9, 8, RefScan::None),
        Err(TypeInstanceShapeError::UnalignedInlineSize)
    );
    assert_eq!(
        ValueStorageLayoutV1::inline(8, 8, RefScan::References(vec![8])),
        Err(TypeInstanceShapeError::Scan(
            RefScanValidationError::OutOfBounds {
                offset: 8,
                size: 8,
                extent: 8
            }
        ))
    );
    assert!(ValueStorageLayoutV1::inline(8, 1, RefScan::References(vec![0])).is_err());
}

#[test]
fn array_stride_has_one_authority_and_zst_has_no_scan() {
    let value = ValueStorageLayoutV1::inline(16, 16, RefScan::References(vec![8])).unwrap();
    let array = ArrayElementStorageV1::from_value(&value);
    let ArrayElementStorageKindV1::Inline {
        stride,
        alignment,
        scan,
    } = array.kind()
    else {
        panic!("nonzero value");
    };
    assert_eq!(stride.get(), value.byte_size());
    assert_eq!(alignment, value.alignment());
    assert_eq!(scan, value.nonzero().unwrap().scan());
    let zst = ValueStorageLayoutV1::zero_sized(16).unwrap();
    assert_eq!(zst.byte_size(), 0);
    assert_eq!(zst.nonzero(), None);
}

#[test]
fn align_up_rejects_overflow_including_high_power_of_two() {
    assert_eq!(
        NonZeroPow2::new(16).unwrap().align_up(u64::MAX),
        Err(TypeInstanceShapeError::SizeOverflow)
    );
    let high = NonZeroPow2::new(1_u64 << 63).unwrap();
    assert_eq!(high.align_up(1).unwrap(), 1_u64 << 63);
    assert_eq!(
        high.align_up((1_u64 << 63) + 1),
        Err(TypeInstanceShapeError::SizeOverflow)
    );
}
