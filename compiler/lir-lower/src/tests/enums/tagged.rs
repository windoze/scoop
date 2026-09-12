//! Tagged enum C layout and recursive GC scan contracts.

use super::*;

#[test]
fn c_layout_keeps_packing_alignment_offsets_and_identity() {
    let mut b = Builder::new();
    let inner = b.c_strukt(
        "Inner",
        mir::MirCLayoutValue::A8,
        mir::MirCLayoutValue::A1,
        false,
        &[("flag", mir::Type::Boolean), ("value", INT)],
    );
    let outer = b.c_strukt(
        "Outer",
        mir::MirCLayoutValue::A16,
        mir::MirCLayoutValue::A2,
        true,
        &[
            ("tag", mir::Type::Boolean),
            ("inner", mir::Type::Struct(inner)),
            ("tail", INT),
        ],
    );
    let wrapped = b.enums.alloc(mir::EnumDef {
        name: "Wrapped".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::Struct(outer),
                }],
            },
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Number".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: INT,
                }],
            },
        ],
    });
    let outer_array = b.array("Array<Outer>", mir::Type::Struct(outer));
    let wrapped_array = b.array("Array<Wrapped>", mir::Type::Enum(wrapped, Vec::new()));
    let mut locals = Arena::new();
    locals.alloc(local("values", outer_array));
    locals.alloc(local("wrapped", wrapped_array));
    let main = b.main(locals, vec![]);
    let source = b.finish(main);
    let outer_exact = source
        .meta
        .source_exact_types
        .get(&mir::Type::Struct(outer))
        .expect("C-layout struct has an exact identity")
        .identity_record()
        .id();
    let module = lower(&source);

    let inner_def = &module.structs[struct_def_id(inner)];
    assert_eq!((inner_def.size, inner_def.align), (8, 8));
    assert_eq!(
        inner_def
            .c_fields()
            .expect("Inner is a C-layout struct")
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>(),
        [
            lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
            lir::FieldLayout {
                offset: 1,
                access_align: 1,
            },
        ]
    );

    let outer_def = &module.structs[struct_def_id(outer)];
    assert_eq!(
        outer_def.c_fields().expect("Outer is a C-layout struct")[1]
            .ty
            .storage_type(),
        lir::LirType::Struct(struct_def_id(inner))
    );
    assert_eq!((outer_def.size, outer_def.align), (16, 16));
    assert_eq!(
        outer_def
            .c_fields()
            .expect("Outer is a C-layout struct")
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>(),
        [
            lir::FieldLayout {
                offset: 0,
                access_align: 1,
            },
            lir::FieldLayout {
                offset: 2,
                access_align: 2,
            },
            lir::FieldLayout {
                offset: 10,
                access_align: 2,
            },
        ]
    );
    assert!(outer_def.interior_mutable);
    assert_eq!(
        outer_def.c_layout(),
        Some(lir::LirCLayoutContract {
            aligned: lir::LirCLayoutValue::A16,
            packed: lir::LirCLayoutValue::A2,
        })
    );

    let outer_layout = layout_values(&module)
        .find(|layout| layout.name == "Outer")
        .expect("Outer layout");
    assert_eq!(
        outer_layout.identity,
        lir::LayoutIdentity::c_value(
            outer_exact,
            lir::LirTargetProfile::DARWIN_AARCH64,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
    assert_eq!((outer_layout.size, outer_layout.align), (16, 16));
    assert_eq!(
        outer_layout.fields,
        outer_def
            .c_fields()
            .expect("Outer is a C-layout struct")
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>()
    );
    assert!(outer_layout.interior_mutable);
    let array_layout = array_metadata(&module, "Array<Outer>");
    assert_eq!(
        (array_layout.element_size, array_layout.element_align),
        (16, 16)
    );
    let wrapped_layout = layout_values(&module)
        .find(|layout| layout.name == "Wrapped")
        .expect("enum layout");
    assert_eq!((wrapped_layout.size, wrapped_layout.align), (32, 16));
    let wrapped_array = array_metadata(&module, "Array<Wrapped>");
    assert_eq!(
        (wrapped_array.element_size, wrapped_array.element_align),
        (32, 16)
    );
    assert!(lir::dump(&module).contains(
            "layout-meta Outer c-layout(aligned=16,packed=2) fields=[0@1,2@2,10@2] interior-mutable=true"
        ));
}

#[test]
fn recursive_scans_preserve_tagged_enums_in_aggregates_and_arrays() {
    let mut b = Builder::new();
    // enum Msg { Text(String), Pair(Boolean, String), Empty }
    let msg = b.enums.alloc(mir::EnumDef {
        name: "Msg".to_string(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![
            mir::VariantDef {
                name: "Text".to_string(),
                gc_free: false,
                fields: vec![mir::Field {
                    name: "value".to_string(),
                    ty: mir::Type::String,
                }],
            },
            mir::VariantDef {
                name: "Pair".to_string(),
                gc_free: false,
                fields: vec![
                    mir::Field {
                        name: "flag".to_string(),
                        ty: mir::Type::Boolean,
                    },
                    mir::Field {
                        name: "s".to_string(),
                        ty: mir::Type::String,
                    },
                ],
            },
            mir::VariantDef {
                name: "Empty".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });
    // A niche enum inside a struct: the value itself is the
    // reference.
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let _s = b.strukt(
        "S",
        &[("o", mir::Type::Enum(option_s, vec![mir::Type::String]))],
    );
    let msg_ty = mir::Type::Enum(msg, Vec::new());
    // Nested { flag: Boolean @0, msg: Msg @8 }. Msg's fixed ref
    // offsets compose without retaining or reading its tag.
    let nested = b.strukt(
        "Nested",
        &[("flag", mir::Type::Boolean), ("msg", msg_ty.clone())],
    );
    // Holder { head: String @16, nested: Nested @24 } combines an
    // unconditional reference with the nested enum scan.
    b.class(
        "Holder",
        None,
        &[
            ("head", mir::Type::String),
            ("nested", mir::Type::Struct(nested)),
        ],
        empty_vtable(),
        vec![],
    );
    let messages_array = b.array("Array<Msg>", msg_ty.clone());
    let nested_array = b.array("Array<Nested>", mir::Type::Struct(nested));
    let mut locals = Arena::new();
    locals.alloc(local("messages", messages_array));
    locals.alloc(local("nestedValues", nested_array));
    let main = b.main(locals, Vec::new());
    let module = lower(&b.finish(main));

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };

    // Msg has two ref-bearing variants, so Text and Pair receive
    // disjoint slots. Empty is the zero-sized shared pure region.
    let msg_layout = by_name("Msg");
    assert_eq!((msg_layout.size, msg_layout.align), (32, 8));
    let lir::LayoutKind::Enum { scan } = &msg_layout.kind else {
        panic!("an enum layout keeps fixed scan offsets")
    };
    assert_eq!(*scan, lir::RefScan::References(vec![8, 24]));
    let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, msg).repr else {
        panic!("Msg is tagged")
    };
    assert_ne!(variants[0].slot_offset, variants[1].slot_offset);
    assert_eq!(variants[2].slot_offset, 8);
    assert_eq!(
        variants[0].fields,
        [lir::EnumFieldRepr {
            ty: lir::LirType::Ptr(lir::PointerKind::Managed),
            offset: 8,
        }]
    );
    assert_eq!(
        variants[1]
            .fields
            .iter()
            .map(|field| field.offset)
            .collect::<Vec<_>>(),
        [16, 24]
    );

    // The niche layout: the payload variant is the reference
    // itself; the unit variant has none.
    let option_layout = by_name("Option$S");
    assert_eq!((option_layout.size, option_layout.align), (8, 8));
    let lir::LayoutKind::Enum { scan } = &option_layout.kind else {
        panic!("an enum layout keeps fixed scan offsets")
    };
    assert_eq!(*scan, lir::RefScan::References(vec![0]));

    // S { o: Option<String> }: the niche value at offset 0 is the
    // struct's reference field.
    let s_layout = by_name("S");
    assert_eq!((s_layout.size, s_layout.align), (8, 8));
    assert_eq!(plain_refs(s_layout), [0]);

    let nested_layout = by_name("Nested");
    assert_eq!((nested_layout.size, nested_layout.align), (40, 8));
    assert_eq!(
        nested_layout.kind,
        lir::LayoutKind::Plain {
            scan: lir::RefScan::References(vec![16, 32]),
        }
    );

    let holder_td = descriptor_values(&module)
        .find(|td| td.name == "Holder")
        .expect("Holder TypeDescriptor");
    assert_eq!((holder_td.size, holder_td.align), (64, 8));
    assert_eq!(
        *fixed_scan(holder_td),
        lir::RefScan::References(vec![16, 40, 56])
    );

    let array_scan = |name: &str| {
        array_scan(descriptor(
            &module,
            array_metadata(&module, name).type_descriptor,
        ))
        .clone()
    };
    assert_eq!(
        array_scan("Array<Msg>"),
        lir::RefScan::References(vec![8, 24])
    );
    assert_eq!(
        array_scan("Array<Nested>"),
        lir::RefScan::References(vec![16, 32])
    );
}
