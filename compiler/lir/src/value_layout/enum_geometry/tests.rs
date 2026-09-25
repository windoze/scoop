use super::*;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

fn geometry(size: u64, alignment: u64) -> StorageGeometryV1 {
    StorageGeometryV1::new(TARGET, size, alignment).unwrap()
}

#[test]
fn independent_pure_maxima_do_not_pad_the_shared_region_before_dedicated_slots() {
    let wide = [geometry(16, 16)];
    let triple = [geometry(8, 8); 3];
    let reference = [geometry(8, 8)];
    let value = EnumStorageGeometryV1::tagged(
        TARGET,
        &[
            EnumVariantGeometryInputV1 {
                fields: &wide,
                gc_free: true,
            },
            EnumVariantGeometryInputV1 {
                fields: &triple,
                gc_free: true,
            },
            EnumVariantGeometryInputV1 {
                fields: &reference,
                gc_free: false,
            },
        ],
    )
    .unwrap();
    assert_eq!(value.storage(), geometry(48, 16));
    assert_eq!(
        (
            value.tag_layout().offset(),
            value.tag_layout().byte_size(),
            value.tag_layout().alignment().get()
        ),
        (0, 8, 8)
    );
    assert_eq!(
        (
            value.pure_region().offset(),
            value.pure_region().byte_size(),
            value.pure_region().alignment().get()
        ),
        (16, 24, 16)
    );
    assert_eq!(value.variants()[0].storage(), geometry(16, 16));
    assert_eq!(value.variants()[1].storage(), geometry(24, 8));
    assert_eq!(value.variants()[2].fields()[0].offset(), 40);
    assert!(matches!(
        value.variants()[0].slot(),
        EnumVariantSlotV1::SharedPure(_)
    ));
    assert!(matches!(
        value.variants()[2].slot(),
        EnumVariantSlotV1::Dedicated(_)
    ));
}

#[test]
fn zero_sized_fields_are_elided_in_shared_and_dedicated_variants() {
    let pure = [geometry(1, 1), geometry(0, 16), geometry(8, 8)];
    let refs = [geometry(0, 16), geometry(8, 8), geometry(0, 1)];
    let value = EnumStorageGeometryV1::tagged(
        TARGET,
        &[
            EnumVariantGeometryInputV1 {
                fields: &pure,
                gc_free: true,
            },
            EnumVariantGeometryInputV1 {
                fields: &refs,
                gc_free: false,
            },
            EnumVariantGeometryInputV1 {
                fields: &refs,
                gc_free: false,
            },
        ],
    )
    .unwrap();
    let offsets = value
        .variants()
        .iter()
        .map(|variant| {
            variant
                .fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        offsets,
        vec![vec![16, 0, 24], vec![0, 32, 0], vec![0, 48, 0]]
    );
    assert_eq!(value.storage(), geometry(64, 16));
    assert_eq!(value.variants()[1].fields()[0].access_alignment().get(), 16);
}

#[test]
fn empty_variants_keep_the_tag_and_do_not_allocate_payload_bytes() {
    let zst = [geometry(0, 16)];
    let value = EnumStorageGeometryV1::tagged(
        TARGET,
        &[
            EnumVariantGeometryInputV1 {
                fields: &[],
                gc_free: true,
            },
            EnumVariantGeometryInputV1 {
                fields: &zst,
                gc_free: true,
            },
        ],
    )
    .unwrap();
    assert_eq!(value.storage(), geometry(16, 16));
    assert_eq!(value.pure_region().offset(), 16);
    assert_eq!(value.pure_region().byte_size(), 0);
    assert_eq!(value.variants()[1].fields()[0].offset(), 0);
    assert_eq!(
        EnumStorageGeometryV1::tagged(TARGET, &[])
            .unwrap()
            .storage(),
        geometry(8, 8)
    );
}

#[test]
fn tagged_geometry_propagates_extent_overflow() {
    let huge = [geometry(TARGET.contract().maximum_managed_object_size(), 1)];
    assert!(matches!(
        EnumStorageGeometryV1::tagged(
            TARGET,
            &[EnumVariantGeometryInputV1 {
                fields: &huge,
                gc_free: true
            }]
        ),
        Err(EnumStorageGeometryErrorV1::Shape(
            TypeInstanceShapeError::ManagedObjectTooLarge { .. }
        ))
    ));
}
