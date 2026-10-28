use super::*;

#[test]
fn static_enum_constants_lower_payloads_and_string_references() {
    let mut mir_enums = Arena::new();
    let enumeration = mir_enums.alloc(mir::EnumDef {
        representation: mir::EnumRepresentation::Algebraic,
        name: "Payload".into(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![test_variant(
            "Data".into(),
            false,
            vec![
                mir::Field {
                    name: "number".into(),
                    ty: mir::Type::Integer(mir::IntegerKind::SIGNED_32),
                },
                mir::Field {
                    name: "text".into(),
                    ty: mir::Type::String,
                },
            ],
        )],
    });
    let variant = mir::MirVariantRef::new(&mir_enums, enumeration, 0).unwrap();
    let mut enums = lir::EnumDefs::default();
    let target = enums.alloc(lir::EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Payload",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Payload".into(),
        repr: lir::EnumRepr::Tagged {
            variants: vec![lir::EnumVariantRepr {
                fields: vec![
                    lir::EnumFieldRepr {
                        ty: lir::LirType::I32,
                        offset: 8,
                    },
                    lir::EnumFieldRepr {
                        ty: lir::MANAGED_PTR,
                        offset: 16,
                    },
                ],
                slot_offset: 8,
                slot_size: 16,
                slot_align: 8,
                gc_free: false,
            }],
            size: 24,
            align: 8,
        },
        scan: lir::RefScan::References(vec![16]),
    });
    let string = mir::StringConstId::from_raw(3.into());
    let global = lir::GlobalId::from_raw(7.into());
    let image = lower_constant_image(
        &mir::MirConstantImage::Enum {
            variant,
            fields: vec![
                mir::MirConstantImage::Integer(mir::MirIntegerConstant::Signed32(42)),
                mir::MirConstantImage::String(string),
            ],
        },
        &enums,
        &HashMap::from([(string, global)]),
    );
    assert_eq!(
        image,
        lir::LirConstantImage::Enum {
            variant: enums.variant_ref(target, 0).unwrap(),
            fields: vec![
                lir::LirConstantImage::Integer(lir::LirIntegerConstant::Signed32(42)),
                lir::LirConstantImage::GlobalPointer {
                    global,
                    kind: lir::PointerKind::Managed
                }
            ],
        }
    );
}
