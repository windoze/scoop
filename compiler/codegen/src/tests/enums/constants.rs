use super::*;
use inkwell::values::AnyValue;

#[test]
fn static_enum_constants_encode_niche_tagged_and_inactive_reference_slots() {
    let mut source = enum_module();
    let context = Context::create();
    let module = context.create_module("enum_constants");
    let address_space = ManagedAddressSpace::MOVING_GC;
    let target_data = inkwell::targets::TargetData::create(
        source.meta.target_profile.canonical_llvm_data_layout(),
    );
    let text = module.add_global(context.i8_type(), Some(address_space.inkwell()), "text");
    text.set_initializer(&context.i8_type().const_zero());
    let mut globals = BTreeMap::new();
    let key = scoop_lir::GlobalId::from_raw(0.into());
    globals.insert(key, text);
    let variants = source.enums.iter().map(|(id, _)| id).collect::<Vec<_>>();
    let shape = variants[0];
    let option = variants[1];
    let nested = source.enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Option<Option<String>>",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Option<Option<String>>".into(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::Enum(option),
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 8,
                    slot_align: 8,
                    gc_free: false,
                },
            ],
            size: 16,
            align: 8,
        },
        scan: RefScan::References(vec![8]),
    });
    let string = LirConstantImage::GlobalPointer {
        global: key,
        kind: PointerKind::Managed,
    };
    let render = |enumeration, index, fields| {
        let value = crate::llvm_types::llvm_constant(
            &context,
            &source.structs,
            &source.enums,
            &globals,
            address_space,
            &target_data,
            &LirType::Enum(enumeration),
            &LirConstantImage::Enum {
                variant: source.enums.variant_ref(enumeration, index).unwrap(),
                fields,
            },
        )
        .unwrap();
        let name = format!("value_{}_{}", enumeration.into_raw(), index);
        let global = module.add_global(value.get_type(), None, &name);
        global.set_initializer(&value);
        value.print_to_string().to_string()
    };
    let some = render(option, 1, vec![string.clone()]);
    assert!(some.contains("@text"), "{some}");
    let none = render(option, 0, Vec::new());
    assert!(none.contains("null"), "{none}");
    let tagged = render(
        shape,
        2,
        vec![
            LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(42)),
            string.clone(),
        ],
    );
    assert!(
        tagged.contains("i64 2") && tagged.contains("@text") && tagged.contains('*'),
        "{tagged}"
    );
    let pure = render(
        shape,
        1,
        vec![LirConstantImage::Integer(
            scoop_lir::LirIntegerConstant::Signed64(7),
        )],
    );
    assert!(
        pure.contains("i64 1") && pure.contains("null") && !pure.contains("@text"),
        "{pure}"
    );
    let nested_none = render(
        nested,
        1,
        vec![LirConstantImage::Enum {
            variant: source.enums.variant_ref(option, 0).unwrap(),
            fields: Vec::new(),
        }],
    );
    assert!(
        nested_none.contains("i64 1") && nested_none.contains("null"),
        "{nested_none}"
    );
    let nested_some = render(
        nested,
        1,
        vec![LirConstantImage::Enum {
            variant: source.enums.variant_ref(option, 1).unwrap(),
            fields: vec![string],
        }],
    );
    assert!(
        nested_some.contains("i64 1") && nested_some.contains("@text"),
        "{nested_some}"
    );
    module.verify().unwrap();
}

#[test]
fn static_tuple_constants_emit_nested_padding_and_references_inside_enums() {
    let mut source = enum_module();
    let context = Context::create();
    let module = context.create_module("tuple_constants");
    let address_space = ManagedAddressSpace::MOVING_GC;
    let target_data = inkwell::targets::TargetData::create(
        source.meta.target_profile.canonical_llvm_data_layout(),
    );
    module.set_data_layout(&target_data.get_data_layout());
    let text = module.add_global(context.i8_type(), Some(address_space.inkwell()), "text");
    text.set_initializer(&context.i8_type().const_zero());
    let key = scoop_lir::GlobalId::from_raw(0.into());
    let globals = BTreeMap::from([(key, text)]);
    let tuple_ty = LirType::Aggregate(vec![
        LirType::I8,
        LirType::Aggregate(vec![
            LirType::Aggregate(Vec::new()),
            LirType::F64,
            LirType::Ptr(PointerKind::Managed),
        ]),
        LirType::I16,
    ]);
    let tuple = LirConstantImage::Aggregate(vec![
        LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Unsigned8(0xab)),
        LirConstantImage::Aggregate(vec![
            LirConstantImage::Aggregate(Vec::new()),
            LirConstantImage::Float(scoop_lir::LirFloatConstant::F64(1.5_f64.to_bits())),
            LirConstantImage::GlobalPointer {
                global: key,
                kind: PointerKind::Managed,
            },
        ]),
        LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Unsigned16(0x1234)),
    ]);
    let enumeration = source.enums.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "TuplePayload",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "TuplePayload".into(),
        repr: EnumRepr::Tagged {
            variants: vec![EnumVariantRepr {
                fields: vec![EnumFieldRepr {
                    ty: tuple_ty.clone(),
                    offset: 8,
                }],
                slot_offset: 8,
                slot_size: 32,
                slot_align: 8,
                gc_free: false,
            }],
            size: 40,
            align: 8,
        },
        scan: RefScan::References(vec![24]),
    });
    for (name, ty, image) in [
        ("tuple", tuple_ty, tuple.clone()),
        (
            "packet",
            LirType::Enum(enumeration),
            LirConstantImage::Enum {
                variant: source.enums.variant_ref(enumeration, 0).unwrap(),
                fields: vec![tuple],
            },
        ),
        (
            "unit",
            LirType::Aggregate(Vec::new()),
            LirConstantImage::Aggregate(Vec::new()),
        ),
    ] {
        let value = crate::llvm_types::llvm_constant(
            &context,
            &source.structs,
            &source.enums,
            &globals,
            address_space,
            &target_data,
            &ty,
            &image,
        )
        .unwrap();
        let global = module.add_global(value.get_type(), None, name);
        global.set_initializer(&value);
        if name != "unit" {
            assert!(value.print_to_string().to_string().contains("@text"));
        }
    }
    module.verify().unwrap();
}
