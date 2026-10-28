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
                    fields: vec![
                        crate::EnumFieldRepr {
                            ty: LirType::F32,
                            offset: 8,
                        },
                        crate::EnumFieldRepr {
                            ty: LirType::Enum(option),
                            offset: 16,
                        },
                    ],
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
    globals.alloc(storage_global(
        "packet",
        LirType::Enum(packet),
        RefScan::References(vec![16]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Enum {
                variant: enums.variant_ref(packet, 1).unwrap(),
                fields: vec![
                    LirConstantImage::Float(crate::LirFloatConstant::F32(1.5_f32.to_bits())),
                    LirConstantImage::Enum {
                        variant: enums.variant_ref(option, 1).unwrap(),
                        fields: vec![LirConstantImage::GlobalPointer {
                            global: text,
                            kind: PointerKind::Managed,
                        }],
                    },
                ],
            },
        },
    ));
    let plans = StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        crate::target_fixtures::DARWIN_AARCH64,
        &globals,
        &crate::StructDefs::default(),
        &enums,
    )
    .unwrap();
    let plan = &plans.storages()[0];
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
}
