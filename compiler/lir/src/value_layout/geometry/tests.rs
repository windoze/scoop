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
