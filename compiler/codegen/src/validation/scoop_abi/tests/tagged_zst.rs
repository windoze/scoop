use super::*;

fn tagged_module(fields: Vec<scoop_lir::EnumFieldRepr>, referenced: bool) -> (Module, EnumDefId) {
    let mut enums = EnumDefs::default();
    let size = if fields.len() == 1 { 8 } else { 16 };
    let scan = if referenced {
        RefScan::References(vec![8])
    } else {
        RefScan::None
    };
    let id = enums.alloc(scoop_lir::EnumDef {
        name: "TaggedZst".into(),
        repr: EnumRepr::Tagged {
            variants: vec![scoop_lir::EnumVariantRepr {
                fields,
                slot_offset: 8,
                slot_size: size - 8,
                slot_align: if size == 8 { 1 } else { 8 },
                gc_free: !referenced,
            }],
            size,
            align: 8,
        },
        scan,
    });
    (module_with_types(StructDefs::default(), enums), id)
}

fn zst(offset: u64) -> scoop_lir::EnumFieldRepr {
    scoop_lir::EnumFieldRepr {
        ty: LirType::Aggregate(vec![]),
        offset,
    }
}

#[test]
fn enum_with_only_zst_payload_retains_its_nonzero_tag() {
    let (module, id) = tagged_module(vec![zst(0)], false);
    let facts = AbiMetadataValidator::new(&module)
        .enum_facts(id, "test enum")
        .unwrap();
    assert_eq!(
        facts,
        StorageFacts {
            size: 8,
            align: 8,
            scan: RefScan::None
        }
    );
}

#[test]
fn elided_payload_offsets_remain_zero_around_stored_and_reference_fields() {
    for ty in [LirType::I64, scoop_lir::MANAGED_PTR] {
        let referenced = ty == scoop_lir::MANAGED_PTR;
        let fields = vec![zst(0), scoop_lir::EnumFieldRepr { ty, offset: 8 }, zst(0)];
        let (module, id) = tagged_module(fields, referenced);
        let facts = AbiMetadataValidator::new(&module)
            .enum_facts(id, "test enum")
            .unwrap();
        assert_eq!(facts.size, 16);
        assert_eq!(facts.align, 8);
        assert_eq!(
            facts.scan,
            if referenced {
                RefScan::References(vec![8])
            } else {
                RefScan::None
            }
        );
    }
}

#[test]
fn zst_fields_reject_payload_relative_offsets() {
    let (module, id) = tagged_module(vec![zst(8)], false);
    let error = AbiMetadataValidator::new(&module)
        .enum_facts(id, "test enum")
        .unwrap_err();
    assert!(
        error
            .0
            .contains("field 0 offset 8 disagrees with target offset 0"),
        "{error}"
    );
}

#[test]
fn nonzero_fields_still_require_their_actual_slot_offset() {
    let fields = vec![
        zst(0),
        scoop_lir::EnumFieldRepr {
            ty: LirType::I64,
            offset: 0,
        },
    ];
    let (module, id) = tagged_module(fields, false);
    let error = AbiMetadataValidator::new(&module)
        .enum_facts(id, "test enum")
        .unwrap_err();
    assert!(
        error
            .0
            .contains("field 1 offset 0 disagrees with target offset 8"),
        "{error}"
    );
}
