use super::*;

/// The LIR enum definition transposed from a MIR enum (the arenas
/// align 1:1).
fn edef(module: &lir::Module, id: mir::EnumId) -> &lir::EnumDef {
    &module.enums[lir::EnumDefId::from_raw(id.into_raw())]
}

/// main holding `o: Option<T>` through a None / tag / field /
/// wrap round-trip; shared shell of the two representation tests.
fn option_round_trip(name: &str, payload: mir::Type) -> mir::Module {
    let mut b = Builder::new();
    let option = b.option_enum(name, payload.clone());
    let option_ty = mir::Type::Enum(option, vec![payload.clone()]);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let t = locals.alloc(local("t", mir::Type::Int));
    let p = locals.alloc(local("p", payload.clone()));
    let o2 = locals.alloc(local("o2", option_ty.clone()));
    let main = b.main(
        locals,
        vec![
            // None
            val_decl(
                o,
                expr(
                    option_ty.clone(),
                    mir::ExprKind::VariantConstruct {
                        variant: 1,
                        fields: Vec::new(),
                    },
                ),
            ),
            val_decl(
                t,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::EnumTag(Box::new(local_expr(o, option_ty.clone()))),
                ),
            ),
            val_decl(
                p,
                expr(
                    payload.clone(),
                    mir::ExprKind::EnumField {
                        operand: Box::new(local_expr(o, option_ty.clone())),
                        variant: 0,
                        index: 0,
                    },
                ),
            ),
            val_decl(
                o2,
                expr(
                    option_ty,
                    mir::ExprKind::VariantConstruct {
                        variant: 0,
                        fields: vec![local_expr(p, payload)],
                    },
                ),
            ),
        ],
    );
    b.finish(main)
}

#[test]
fn option_of_string_uses_the_niche_representation() {
    // Option<String>: the payload maps to `Ptr`, so the value is
    // the pointer itself with None = null (spec 7.4).
    let module = lower(&option_round_trip("Option$S", mir::Type::String));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  enum Option$S niche(payload_variant=0)
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: ptr<managed>
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp1 live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr<managed>
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$S size=8 align=8 enum-scan=refs[0]
  entry @scoop_main
"###);
}

#[test]
fn option_of_raw_pointer_uses_a_niche_without_gc_scanning() {
    let mut builder = Builder::new();
    let option = builder.option_enum("Option$P", mir::Type::Ptr(Box::new(mir::Type::Int)));
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));

    assert!(matches!(
        edef(&module, option).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    let layout = layout_values(&module)
        .find(|layout| layout.name == "Option$P")
        .expect("raw pointer option layout");
    let lir::LayoutKind::Enum { scan } = &layout.kind else {
        panic!("Option<Ptr<Int>> must retain its enum layout identity")
    };
    assert_eq!(*scan, lir::RefScan::None);
}

#[test]
fn option_of_int_uses_the_tagged_representation() {
    // Option<Int>: the `{ i64 tag, [8 x i8] payload }` tagged
    // form — size 16, align 8.
    let module = lower(&option_round_trip("Option$I", mir::Type::Int));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: i64
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp1 live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : i64
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
"###);
}

#[test]
fn niche_detection_requires_option_isomorphic_pointer_shape() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let array_int = b.array("Array<Int>", mir::Type::Int);
    let option_array = b.option_enum("Option$Array$I", array_int);
    let option_i = b.option_enum("Option$I", mir::Type::Int);
    // Reversed declaration order: the payload variant comes second.
    let flip = b.enums.alloc(mir::EnumDef {
        name: "Flip".to_string(),
        gc_free: false,
        variants: vec![
            mir::VariantDef {
                name: "Naught".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
            mir::VariantDef {
                name: "Value".to_string(),
                gc_free: false,
                fields: vec![mir::Field {
                    name: "_1".to_string(),
                    ty: mir::Type::String,
                }],
            },
        ],
    });
    // Two variants, but the payload variant has two fields: tagged.
    let pair_or_none = b.enums.alloc(mir::EnumDef {
        name: "PairOrNone".to_string(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Pair".to_string(),
                gc_free: true,
                fields: vec![
                    mir::Field {
                        name: "_1".to_string(),
                        ty: mir::Type::Int,
                    },
                    mir::Field {
                        name: "_2".to_string(),
                        ty: mir::Type::Int,
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
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(&b.finish(main));

    assert!(matches!(
        edef(&module, option_s).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    assert!(matches!(
        edef(&module, option_array).repr,
        lir::EnumRepr::Niche { payload_variant: 0 }
    ));
    assert!(matches!(
        edef(&module, flip).repr,
        lir::EnumRepr::Niche { payload_variant: 1 }
    ));
    let lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    } = &edef(&module, option_i).repr
    else {
        panic!("Option<Int> must use the tagged representation")
    };
    assert_eq!(
        variants[0].fields,
        [lir::EnumFieldRepr {
            ty: lir::LirType::I64,
            offset: 8,
        }]
    );
    assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
    assert!(variants.iter().all(|variant| variant.gc_free));
    assert_eq!((*size, *align), (16, 8));
    let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, pair_or_none).repr else {
        panic!("PairOrNone must be tagged")
    };
    assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
    assert!(variants.iter().all(|variant| variant.gc_free));
}

#[test]
fn c_layout_keeps_packing_alignment_offsets_and_identity() {
    let mut b = Builder::new();
    let inner = b.c_strukt(
        "Inner",
        8,
        1,
        false,
        &[("flag", mir::Type::Boolean), ("value", mir::Type::Int)],
    );
    let outer = b.c_strukt(
        "Outer",
        16,
        2,
        true,
        &[
            ("tag", mir::Type::Boolean),
            ("inner", mir::Type::Struct(inner)),
            ("tail", mir::Type::Int),
        ],
    );
    let wrapped = b.enums.alloc(mir::EnumDef {
        name: "Wrapped".to_string(),
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
                    ty: mir::Type::Int,
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
    let module = lower(&b.finish(main));

    let inner_def = &module.structs[struct_def_id(inner)];
    assert_eq!((inner_def.size, inner_def.align), (16, 8));
    assert_eq!(
        inner_def
            .fields
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
        outer_def.fields[1].ty,
        lir::LirType::Struct(struct_def_id(inner))
    );
    assert_eq!((outer_def.size, outer_def.align), (32, 16));
    assert_eq!(
        outer_def
            .fields
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
                offset: 18,
                access_align: 2,
            },
        ]
    );
    assert!(outer_def.interior_mutable);

    let outer_layout = layout_values(&module)
        .find(|layout| layout.name == "Outer")
        .expect("Outer layout");
    assert_eq!((outer_layout.size, outer_layout.align), (32, 16));
    assert_eq!(
        outer_layout.fields,
        outer_def
            .fields
            .iter()
            .map(|field| field.layout)
            .collect::<Vec<_>>()
    );
    assert!(outer_layout.interior_mutable);
    let array_layout = array_metadata(&module, "Array<Outer>");
    assert_eq!(
        (array_layout.element_size, array_layout.element_align),
        (32, 16)
    );
    let wrapped_layout = layout_values(&module)
        .find(|layout| layout.name == "Wrapped")
        .expect("enum layout");
    assert_eq!((wrapped_layout.size, wrapped_layout.align), (48, 16));
    let wrapped_array = array_metadata(&module, "Array<Wrapped>");
    assert_eq!(
        (wrapped_array.element_size, wrapped_array.element_align),
        (48, 16)
    );
    assert!(lir::dump(&module).contains(
            "layout-meta Outer c-layout(aligned=16,packed=2) fields=[0@1,2@2,18@2] interior-mutable=true"
        ));
}

#[test]
fn recursive_scans_preserve_tagged_enums_in_aggregates_and_arrays() {
    let mut b = Builder::new();
    // enum Msg { Text(String), Pair(Boolean, String), Empty }
    let msg = b.enums.alloc(mir::EnumDef {
        name: "Msg".to_string(),
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
