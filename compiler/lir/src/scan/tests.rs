use std::num::NonZeroU64;

use super::*;
use crate::NonEmptyRefScan;

fn array(length: u64, first: u64, stride: u64, child: RefScan) -> RefScan {
    RefScan::Array {
        length_offset: length,
        first_element_offset: first,
        stride: NonZeroU64::new(stride).unwrap(),
        element: Box::new(NonEmptyRefScan::new(child).unwrap()),
    }
}

#[test]
fn producer_normalization_is_fixed_and_reader_does_not_repair() {
    let first = array(0, 8, 8, RefScan::References(vec![0]));
    let second = array(16, 24, 16, RefScan::References(vec![8]));
    let input = RefScan::Sequence(vec![
        RefScan::None,
        first.clone(),
        RefScan::Sequence(vec![RefScan::References(vec![16, 0, 16]), second.clone()]),
        RefScan::References(vec![8]),
        first,
    ]);
    assert!(CheckedRefScanV1::from_canonical(input.clone()).is_err());
    let normalized = CheckedRefScanV1::normalize(input).unwrap();
    assert_eq!(
        CheckedRefScanV1::normalize(normalized.scan.clone()).unwrap(),
        normalized
    );
    let RefScan::Sequence(parts) = &normalized.scan else {
        panic!("multiple scan kinds remain");
    };
    assert_eq!(parts.len(), 3);
    assert!(parts.contains(&RefScan::References(vec![0, 8, 16])));
    let mut reversed = parts.clone();
    reversed.reverse();
    assert_eq!(
        CheckedRefScanV1::from_canonical(RefScan::Sequence(reversed)),
        Err(RefScanValidationError::NonCanonicalSequence)
    );
}

#[test]
fn canonical_bytes_and_fingerprint_use_runtime_framing() {
    let scan = CheckedRefScanV1::from_canonical(RefScan::References(vec![0, 8])).unwrap();
    let expected = [
        1_u32.to_le_bytes().as_slice(),
        2_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        8_u64.to_le_bytes().as_slice(),
    ]
    .concat();
    assert_eq!(scan.canonical_bytes(), expected);
    let framed = [scoop_wire::byte_span(b"scoop-scan-v1").unwrap(), expected].concat();
    assert_eq!(
        scan.fingerprint().as_array(),
        scoop_wire::sha256(&framed).as_array()
    );
    let none = CheckedRefScanV1::from_canonical(RefScan::None).unwrap();
    assert_eq!(none.canonical_bytes(), &0_u32.to_le_bytes());
}

#[test]
fn arrays_use_dynamic_count_and_translation_preserves_element_base() {
    let scan =
        CheckedRefScanV1::from_canonical(array(0, 8, 16, RefScan::References(vec![8]))).unwrap();
    scan.validate_extent(8, 8).unwrap();
    let moved = scan.translated(16).unwrap();
    assert_eq!(moved.scan, array(16, 24, 16, RefScan::References(vec![8])));
    moved.validate_extent(24, 8).unwrap();
    assert_eq!(
        scan.translated(u64::MAX),
        Err(RefScanValidationError::OffsetOverflow)
    );
}

#[test]
fn reader_rejects_unsorted_duplicate_and_empty_references() {
    for offsets in [vec![8, 0], vec![0, 0]] {
        assert_eq!(
            CheckedRefScanV1::from_canonical(RefScan::References(offsets)),
            Err(RefScanValidationError::UnorderedReferences)
        );
    }
    assert_eq!(
        CheckedRefScanV1::from_canonical(RefScan::References(vec![])),
        Err(RefScanValidationError::EmptyReferences)
    );
}

#[test]
fn range_validation_rejects_overflow_misalignment_and_overlapping_payloads() {
    for (offset, extent, expected) in [
        (
            4,
            16,
            RefScanValidationError::MisalignedReference { offset: 4 },
        ),
        (
            8,
            8,
            RefScanValidationError::OutOfBounds {
                offset: 8,
                size: 8,
                extent: 8,
            },
        ),
        (
            u64::MAX - 7,
            u64::MAX,
            RefScanValidationError::OffsetOverflow,
        ),
    ] {
        let scan = CheckedRefScanV1::from_canonical(RefScan::References(vec![offset])).unwrap();
        assert_eq!(scan.validate_extent(extent, 8), Err(expected));
    }
    let scan = CheckedRefScanV1::normalize(RefScan::Sequence(vec![
        array(0, 16, 8, RefScan::References(vec![0])),
        RefScan::References(vec![24]),
    ]))
    .unwrap();
    assert_eq!(
        scan.validate_extent(32, 8),
        Err(RefScanValidationError::OverlappingRanges)
    );
    let prefix =
        CheckedRefScanV1::from_canonical(array(8, 8, 8, RefScan::References(vec![0]))).unwrap();
    assert_eq!(
        prefix.validate_extent(16, 8),
        Err(RefScanValidationError::ArrayPrefixOverlap)
    );
    let stride =
        CheckedRefScanV1::from_canonical(array(0, 8, 4, RefScan::References(vec![0]))).unwrap();
    assert_eq!(
        stride.validate_extent(8, 8),
        Err(RefScanValidationError::MisalignedArrayStride { stride: 4 })
    );
}

#[test]
fn depth_boundary_is_checked_before_normalization_or_encoding() {
    let mut scan = RefScan::References(vec![0]);
    for _ in 1..64 {
        scan = array(0, 8, 16, scan);
    }
    assert_eq!(
        CheckedRefScanV1::from_canonical(scan.clone())
            .unwrap()
            .usage()
            .depth,
        64
    );
    let scan = array(0, 8, 16, scan);
    assert!(matches!(
        CheckedRefScanV1::normalize(scan),
        Err(RefScanValidationError::BudgetExceeded {
            resource: ScanBudgetResourceV1::Depth,
            ..
        })
    ));
}
