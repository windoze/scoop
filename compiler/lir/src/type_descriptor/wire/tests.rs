use super::*;
use crate::{ArrayElementStorageV1, LirTargetProfile, RefScan, ValueStorageLayoutV1};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn decode(shape: &TypeInstanceShapeV1) -> DecodedTypeInstanceShapeV1 {
    decode_canonical(&encode(shape).unwrap(), DecodeLimits::default()).unwrap()
}

fn shapes() -> Vec<TypeInstanceShapeV1> {
    vec![
        TypeInstanceShapeV1::fixed_object(TARGET, 24, 8, RefScan::References(vec![16])).unwrap(),
        TypeInstanceShapeV1::boxed_value(TARGET, ValueStorageLayoutV1::zero_sized(16).unwrap())
            .unwrap(),
        TypeInstanceShapeV1::boxed_value(
            TARGET,
            ValueStorageLayoutV1::inline(8, 8, RefScan::References(vec![0])).unwrap(),
        )
        .unwrap(),
        TypeInstanceShapeV1::inline_bytes(TARGET).unwrap(),
        TypeInstanceShapeV1::inline_array(TARGET, ArrayElementStorageV1::zero_sized(16).unwrap())
            .unwrap(),
        TypeInstanceShapeV1::inline_array(
            TARGET,
            ArrayElementStorageV1::inline(8, 8, RefScan::References(vec![0])).unwrap(),
        )
        .unwrap(),
        TypeInstanceShapeV1::abstract_ref(),
    ]
}

#[test]
fn shape_wire_round_trips_all_closed_instances_and_inline_storage_branches() {
    for shape in shapes() {
        let decoded = decode(&shape);
        assert_eq!(encode(&decoded).unwrap(), encode(&shape).unwrap());
        decoded.validate_against(&shape, &mut meter()).unwrap();
    }
    assert_eq!(
        encode(&TypeInstanceShapeV1::abstract_ref()).unwrap(),
        [
            0xaa, 1, 5, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0xa1, 0, 1, 10, 0xa1, 0, 1
        ]
    );
}

#[test]
fn shape_reader_rejects_every_changed_scalar_and_both_changed_scans() {
    let shape = TypeInstanceShapeV1::abstract_ref();
    for index in 0..8 {
        let mut decoded = decode(&shape);
        decoded.scalars[index] += 1;
        assert!(matches!(
            decoded.validate_against(&shape, &mut meter()),
            Err(TypeInstanceShapeWireError::ShapeMismatch)
        ));
    }
    let shape = TypeInstanceShapeV1::boxed_value(
        TARGET,
        ValueStorageLayoutV1::inline(8, 8, RefScan::References(vec![0])).unwrap(),
    )
    .unwrap();
    for object in [false, true] {
        let mut decoded = decode(&shape);
        let none: DecodedRefScanV1 =
            decode_canonical(&[0xa1, 0, 1], DecodeLimits::default()).unwrap();
        if object {
            decoded.object_scan = none;
        } else {
            decoded.inline_scan = none;
        }
        assert!(matches!(
            decoded.validate_against(&shape, &mut meter()),
            Err(TypeInstanceShapeWireError::ScanMismatch)
        ));
    }
}

#[test]
fn shape_reader_requires_exact_product_length_and_canonical_scan() {
    let shape = TypeInstanceShapeV1::abstract_ref();
    for length in [0xa9, 0xab] {
        let mut bytes = encode(&shape).unwrap();
        bytes[0] = length;
        assert!(
            decode_canonical::<DecodedTypeInstanceShapeV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
    let mut decoded = decode(&shape);
    decoded.object_scan =
        decode_canonical(&[0xa2, 0, 2, 1, 0x82, 0, 0], DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate_against(&shape, &mut meter()),
        Err(TypeInstanceShapeWireError::Scan(
            MeteredScanValidationError::Scan(crate::RefScanValidationError::UnorderedReferences)
        ))
    ));
}

#[test]
fn shape_refinement_charges_shared_scan_allocations_and_work() {
    let shape = TypeInstanceShapeV1::inline_array(
        TARGET,
        ArrayElementStorageV1::inline(8, 8, RefScan::References(vec![0])).unwrap(),
    )
    .unwrap();
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(
            decode(&shape)
                .validate_against(&shape, &mut BudgetMeter::new(limits))
                .is_err()
        );
    }
}
