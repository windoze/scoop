use super::*;

fn context() -> LoweringContext {
    LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64)
}

fn tagged(enums: &mut lir::EnumDefs, size: u64, scan: lir::RefScan) -> lir::LirType {
    lir::LirType::Enum(enums.alloc(lir::EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "ScanValue",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "ScanValue".to_string(),
        repr: lir::EnumRepr::Tagged {
            variants: Vec::new(),
            size,
            align: 8,
        },
        scan,
    }))
}

#[test]
fn aggregate_zst_alignment_and_managed_leaves_share_checked_placement() {
    let mut structs = lir::StructDefs::default();
    let aligned_zst = structs.alloc_scoop(
        crate::tests::test_physical_exact("Empty", scoop_identity::SourceNominalKind::Struct),
        "Empty".to_string(),
        0,
        16,
        false,
        Vec::new(),
    );
    let enums = lir::EnumDefs::default();
    let fields = vec![
        lir::LirType::I8,
        lir::LirType::Struct(aligned_zst),
        lir::LirType::Ptr(lir::PointerKind::Managed),
        lir::LirType::Aggregate(Vec::new()),
    ];
    assert_eq!(
        layout::lir_aggregate_shape(&context(), &fields, &structs, &enums).unwrap(),
        (vec![0, 0, 8, 0], 16, 16)
    );
    let ty = lir::LirType::Aggregate(fields);
    assert_eq!(
        root_scan(&context(), &ty, &structs, &enums, 16).unwrap(),
        lir::RefScan::References(vec![24])
    );
    let argument = crate::abi::classify_argument(&context(), ty, &structs, &enums).unwrap();
    assert!(matches!(argument, lir::AbiArgument::Indirect(_)));
}

#[test]
fn abi_classification_propagates_invalid_geometry_and_void_value_errors() {
    let mut structs = lir::StructDefs::default();
    let enums = lir::EnumDefs::default();
    let invalid = structs.alloc_scoop(
        crate::tests::test_physical_exact("Invalid", scoop_identity::SourceNominalKind::Struct),
        "Invalid".to_string(),
        8,
        0,
        false,
        Vec::new(),
    );
    assert_eq!(
        crate::abi::classify_argument(&context(), lir::LirType::Struct(invalid), &structs, &enums,),
        Err(StorageLoweringError::Shape(
            lir::TypeInstanceShapeError::ZeroAlignment
        ))
    );
    assert_eq!(
        crate::abi::classify_argument(&context(), lir::LirType::Void, &structs, &enums),
        Err(StorageLoweringError::AbiValue(
            lir::AbiValueError::VoidStorageType
        ))
    );
}

#[test]
fn aggregate_target_extent_and_scan_translation_overflow_return_errors() {
    let mut structs = lir::StructDefs::default();
    let enums = lir::EnumDefs::default();
    let maximum = context()
        .target_profile()
        .contract()
        .maximum_managed_object_size();
    let large = structs.alloc_scoop(
        crate::tests::test_physical_exact("Large", scoop_identity::SourceNominalKind::Struct),
        "Large".to_string(),
        maximum,
        1,
        false,
        Vec::new(),
    );
    let ty = lir::LirType::Aggregate(vec![lir::LirType::Struct(large), lir::LirType::I8]);
    assert!(matches!(
        lir_size_align(&context(), &ty, &structs, &enums),
        Err(StorageLoweringError::Shape(
            lir::TypeInstanceShapeError::ManagedObjectTooLarge { .. }
        ))
    ));
    let ty = lir::LirType::Aggregate(vec![
        lir::LirType::I64,
        lir::LirType::Ptr(lir::PointerKind::Managed),
    ]);
    assert_eq!(
        root_scan(&context(), &ty, &structs, &enums, u64::MAX),
        Err(StorageLoweringError::Scan(
            lir::RefScanValidationError::OffsetOverflow
        ))
    );
}

#[test]
fn zst_abi_cannot_elide_a_value_with_an_out_of_bounds_reference() {
    let structs = lir::StructDefs::default();
    let mut enums = lir::EnumDefs::default();
    let ty = tagged(&mut enums, 0, lir::RefScan::References(vec![0]));
    assert!(matches!(
        crate::abi::classify_signature(&context(), [ty], None, &structs, &enums),
        Err(StorageLoweringError::Scan(
            lir::RefScanValidationError::OutOfBounds { extent: 0, .. }
        ))
    ));
}

#[test]
fn inline_roots_reject_misaligned_and_variable_object_scans() {
    let structs = lir::StructDefs::default();
    let mut enums = lir::EnumDefs::default();
    let ty = tagged(&mut enums, 16, lir::RefScan::References(vec![1]));
    assert_eq!(
        root_scan(&context(), &ty, &structs, &enums, 0),
        Err(StorageLoweringError::Scan(
            lir::RefScanValidationError::MisalignedReference { offset: 1 }
        ))
    );
    let ty = tagged(
        &mut enums,
        16,
        lir::RefScan::Array {
            length_offset: 0,
            first_element_offset: 8,
            stride: std::num::NonZeroU64::new(8).unwrap(),
            element: Box::new(
                lir::NonEmptyRefScan::new(lir::RefScan::References(vec![0])).unwrap(),
            ),
        },
    );
    assert_eq!(
        root_scan(&context(), &ty, &structs, &enums, 0),
        Err(StorageLoweringError::InvalidRepresentation(
            "a safepoint inline value cannot contain a variable object scan"
        ))
    );
}

#[test]
fn recursive_value_roots_return_cycle_error() {
    let mut structs = lir::StructDefs::default();
    let id = structs.alloc_scoop(
        crate::tests::test_physical_exact("Recursive", scoop_identity::SourceNominalKind::Struct),
        "Recursive".to_string(),
        8,
        8,
        false,
        Vec::new(),
    );
    structs.set_scoop_fields(
        id,
        vec![lir::StructField {
            ty: lir::LirType::Struct(id),
            layout: lir::FieldLayout {
                offset: 0,
                access_align: 8,
            },
        }],
    );
    assert_eq!(
        root_scan(
            &context(),
            &lir::LirType::Struct(id),
            &structs,
            &lir::EnumDefs::default(),
            0
        ),
        Err(StorageLoweringError::Scan(
            lir::RefScanValidationError::Cycle
        ))
    );
}
