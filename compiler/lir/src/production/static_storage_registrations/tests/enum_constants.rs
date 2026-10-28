use super::*;

#[test]
fn static_enum_constants_keep_nested_relocations_and_float_bits() {
    let mut enums = crate::EnumDefs::default();
    let option = enums.alloc(crate::EnumDef {
        exact_type: crate::tests::test_physical_exact("Option<String>", SourceNominalKind::Enum),
        name: "Option<String>".into(),
        repr: EnumRepr::Niche {
            kind: crate::NullNicheKind::Managed,
            payload_variant: 1,
        },
        scan: RefScan::References(vec![0]),
    });
    let tuple_type = LirType::Aggregate(vec![
        LirType::F32,
        LirType::Aggregate(Vec::new()),
        LirType::Enum(option),
    ]);
    let packet = enums.alloc(crate::EnumDef {
        exact_type: crate::tests::test_physical_exact("Packet", SourceNominalKind::Enum),
        name: "Packet".into(),
        repr: EnumRepr::Tagged {
            variants: vec![
                crate::EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
                crate::EnumVariantRepr {
                    fields: vec![crate::EnumFieldRepr {
                        ty: tuple_type.clone(),
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 16,
                    slot_align: 8,
                    gc_free: false,
                },
            ],
            size: 24,
            align: 8,
        },
        scan: RefScan::References(vec![16]),
    });
    let mut globals = Arena::new();
    let text = globals.alloc(string_global(0, "payload"));
    let tuple = LirConstantImage::Aggregate(vec![
        LirConstantImage::Float(crate::LirFloatConstant::F32(1.5_f32.to_bits())),
        LirConstantImage::Aggregate(Vec::new()),
        LirConstantImage::Enum {
            variant: enums.variant_ref(option, 1).unwrap(),
            fields: vec![LirConstantImage::GlobalPointer {
                global: text,
                kind: PointerKind::Managed,
            }],
        },
    ]);
    globals.alloc(storage_global(
        "packet",
        LirType::Enum(packet),
        RefScan::References(vec![16]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Enum {
                variant: enums.variant_ref(packet, 1).unwrap(),
                fields: vec![tuple.clone()],
            },
        },
    ));
    let tuple_global = storage_global(
        "tuple",
        tuple_type,
        RefScan::References(vec![8]),
        LirStaticInitialState::EncodedStaticValue { payload: tuple },
    );
    let tuple_id = storage_id(&tuple_global);
    globals.alloc(tuple_global);
    let unit_global = storage_global(
        "unit",
        LirType::Aggregate(Vec::new()),
        RefScan::None,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Aggregate(Vec::new()),
        },
    );
    let unit_id = storage_id(&unit_global);
    globals.alloc(unit_global);
    let plans = StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        crate::target_fixtures::DARWIN_AARCH64,
        &globals,
        &crate::StructDefs::default(),
        &enums,
    )
    .unwrap();
    let plan = plans
        .storages()
        .iter()
        .find(|plan| plan.initial_state().initial_template().len() == 24)
        .unwrap();
    let mut expected = vec![0; 24];
    expected[..8].copy_from_slice(&1_u64.to_le_bytes());
    expected[8..12].copy_from_slice(&1.5_f32.to_bits().to_le_bytes());
    assert_eq!(plan.initial_state().initial_template(), expected);
    let relocations = plan.initial_state().immortal_relocations();
    assert_eq!(relocations.len(), 1);
    assert_eq!(relocations[0].pointer_offset(), 16);
    let GlobalInit::StringConst { identity, .. } = &globals[text].init else {
        unreachable!()
    };
    assert_eq!(relocations[0].target(), identity.identity_record().id());
    let tuple = plans
        .storages()
        .iter()
        .find(|plan| plan.storage() == tuple_id)
        .unwrap();
    assert_eq!(tuple.initial_state().initial_template(), &expected[8..]);
    assert_eq!(
        tuple.initial_state().immortal_relocations()[0].pointer_offset(),
        8
    );
    let unit = plans
        .storages()
        .iter()
        .find(|plan| plan.storage() == unit_id)
        .unwrap();
    assert_eq!(unit.initial_state().initial_template(), &[0]);
    assert!(unit.initial_state().immortal_relocations().is_empty());
}
