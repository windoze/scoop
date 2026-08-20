use super::*;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

#[test]
fn failed_append_keeps_the_checked_prefix_and_cannot_wrap() {
    let prefix = StorageGeometryV1::new(TARGET, i64::MAX as u64, 1).unwrap();
    let mut cursor = StorageLayoutCursorV1::with_prefix(prefix);
    assert!(matches!(
        cursor.push(StorageGeometryV1::new(TARGET, 1, 1).unwrap()),
        Err(TypeInstanceShapeError::ManagedObjectTooLarge { .. })
    ));
    assert_eq!(cursor.finish().unwrap(), prefix);
}

#[test]
fn zst_placement_never_advances_a_complete_base_prefix() {
    let prefix = StorageGeometryV1::new(TARGET, 24, 8).unwrap();
    let mut cursor = StorageLayoutCursorV1::with_prefix(prefix);
    assert_eq!(
        cursor
            .push(StorageGeometryV1::new(TARGET, 0, 16).unwrap())
            .unwrap()
            .offset(),
        0
    );
    assert_eq!(
        cursor
            .push(StorageGeometryV1::new(TARGET, 1, 1).unwrap())
            .unwrap()
            .offset(),
        24
    );
    let result = cursor.finish().unwrap();
    assert_eq!((result.size(), result.alignment().get()), (32, 16));
}

#[test]
fn target_limit_applies_to_tail_padding_before_a_geometry_is_returned() {
    let mut cursor =
        StorageLayoutCursorV1::new(TARGET, StoragePlacementPolicyV1::Ordinary).unwrap();
    cursor
        .push(StorageGeometryV1::new(TARGET, i64::MAX as u64, 1).unwrap())
        .unwrap();
    cursor
        .push(StorageGeometryV1::new(TARGET, 0, 16).unwrap())
        .unwrap();
    assert!(matches!(
        cursor.finish(),
        Err(TypeInstanceShapeError::ManagedObjectTooLarge { .. })
    ));
}

#[test]
fn enum_region_preserves_unpadded_shared_size_and_an_empty_region_position() {
    let mut cursor =
        StorageLayoutCursorV1::new(TARGET, StoragePlacementPolicyV1::Ordinary).unwrap();
    cursor
        .push(StorageGeometryV1::new(TARGET, 8, 8).unwrap())
        .unwrap();
    assert_eq!(
        cursor
            .reserve_region(0, NonZeroPow2::new(8).unwrap())
            .unwrap()
            .offset(),
        8
    );
    assert_eq!(
        cursor
            .reserve_region(24, NonZeroPow2::new(16).unwrap())
            .unwrap()
            .offset(),
        16
    );
    assert_eq!(
        cursor
            .push(StorageGeometryV1::new(TARGET, 8, 8).unwrap())
            .unwrap()
            .offset(),
        40
    );
    let shape = cursor.finish().unwrap();
    assert_eq!((shape.size(), shape.alignment().get()), (48, 16));
}

#[test]
fn rejected_region_alignment_leaves_the_complete_prefix_unchanged() {
    let prefix = StorageGeometryV1::new(TARGET, 24, 8).unwrap();
    let mut cursor = StorageLayoutCursorV1::with_prefix(prefix);
    assert!(matches!(
        cursor.reserve_region(1, NonZeroPow2::new(32).unwrap()),
        Err(TypeInstanceShapeError::ManagedAlignmentTooLarge { .. })
    ));
    assert_eq!(cursor.finish().unwrap(), prefix);
}
