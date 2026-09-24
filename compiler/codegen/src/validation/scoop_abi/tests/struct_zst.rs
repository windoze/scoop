use super::*;

fn field(ty: LirType, offset: u64, access_align: u64) -> scoop_lir::StructField {
    scoop_lir::StructField {
        ty,
        layout: scoop_lir::FieldLayout {
            offset,
            access_align,
        },
    }
}

fn mixed(offset: u64, alignment: u64, pointer_offset: u64) -> (Module, StructDefId) {
    let mut structs = StructDefs::default();
    let empty = structs.alloc_scoop("Empty".into(), 0, 1, false, vec![]);
    let zst = || LirType::Struct(empty);
    let id = structs.alloc_scoop(
        "MixedZst".into(),
        16,
        8,
        false,
        vec![
            field(zst(), 0, 1),
            field(LirType::I8, 0, 1),
            field(zst(), offset, alignment),
            field(scoop_lir::MANAGED_PTR, pointer_offset, 8),
            field(zst(), 0, 1),
        ],
    );
    (module_with_types(structs, EnumDefs::default()), id)
}

#[test]
fn struct_zst_fields_keep_zero_offsets_around_scalars_and_managed_references() {
    let (module, id) = mixed(0, 1, 8);
    let facts = AbiMetadataValidator::new(&module)
        .struct_facts(id, "mixed")
        .unwrap();
    assert_eq!(
        facts,
        StorageFacts {
            size: 16,
            align: 8,
            scan: RefScan::References(vec![8])
        }
    );
    let value = abi_value(LirType::Struct(id), 16, 8, RefScan::References(vec![8]));
    AbiMetadataValidator::new(&module)
        .validate_value(&value, "mixed ABI")
        .unwrap();
}

#[test]
fn struct_zst_fields_reject_physical_offsets_and_incorrect_alignment() {
    for (offset, alignment) in [(1, 1), (8, 1), (0, 8)] {
        let (module, id) = mixed(offset, alignment, 8);
        let error = AbiMetadataValidator::new(&module)
            .struct_facts(id, "mixed")
            .unwrap_err();
        assert!(
            error.0.contains("field 2 layout") && error.0.contains("target layout 0/1"),
            "{error}"
        );
    }
}

#[test]
fn stored_fields_and_scans_remain_checked_in_mixed_zst_structs() {
    let (module, id) = mixed(0, 1, 0);
    let error = AbiMetadataValidator::new(&module)
        .struct_facts(id, "mixed")
        .unwrap_err();
    assert!(
        error.0.contains("field 3 layout 0/8") && error.0.contains("target layout 8/8"),
        "{error}"
    );
    let (module, id) = mixed(0, 1, 8);
    for (size, scan) in [(8, RefScan::References(vec![8])), (16, RefScan::None)] {
        let value = abi_value(LirType::Struct(id), size, 8, scan);
        assert!(
            AbiMetadataValidator::new(&module)
                .validate_value(&value, "mixed ABI")
                .is_err()
        );
    }
}

#[test]
fn aligned_zst_aggregate_fields_do_not_pad_the_storage_cursor() {
    let (offsets, size, align) =
        aggregate_layout([(1, 1), (0, 16), (1, 1)], "aligned ZST").unwrap();
    assert_eq!(offsets, [0, 0, 1]);
    assert_eq!((size, align), (16, 16));
    let (offsets, size, align) = aggregate_layout([(0, 8), (0, 16)], "all ZST").unwrap();
    assert_eq!(offsets, [0, 0]);
    assert_eq!((size, align), (0, 16));
}
