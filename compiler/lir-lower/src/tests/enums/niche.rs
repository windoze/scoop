//! Option-isomorphic niche eligibility and representation.

use super::*;

#[test]
fn option_of_string_uses_the_niche_representation() {
    // Option<String>: the payload maps to `Ptr`, so the value is
    // the pointer itself with None = null (spec 7.4).
    let module = lower(&option_round_trip("Option$S", mir::Type::String));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  enum Option$S niche(kind=managed,payload_variant=0)
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: machine<enum-tag>
    local %2 p: ptr<managed>
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp1 live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : machine<enum-tag>
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr<managed>
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$S size=8 align=8 enum-scan=refs[0]
  entry @scoop_main
"###);
}

#[test]
fn option_of_raw_pointer_uses_a_niche_without_gc_scanning() {
    let mut builder = Builder::new();
    let option = builder.option_enum("Option$P", mir::Type::Ptr(Box::new(INT)));
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));

    assert!(matches!(
        edef(&module, option).repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Raw,
            payload_variant: 0,
        }
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
fn option_of_code_pointer_records_code_niche_provenance() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let mut mir_module = builder.finish(main);
    let signature = mir_module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let option = mir_module.enums.alloc(mir::EnumDef {
        name: "Option$F".to_string(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Some".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "_1".to_string(),
                    ty: mir::Type::FunPtr(signature),
                }],
            },
            mir::VariantDef {
                name: "None".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });

    let module = lower(&mir_module);

    assert!(matches!(
        edef(&module, option).repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Code,
            payload_variant: 0,
        }
    ));
}

#[test]
fn option_of_int_uses_the_tagged_representation() {
    // Option<Int>: the `{ i64 tag, [4 x i8] payload }` tagged
    // form — size 16, align 8.
    let module = lower(&option_round_trip("Option$I", INT));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  enum Option$I tagged size=16 align=8 variants=(i32)@8+4 ()@8+0
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: machine<enum-tag>
    local %2 p: i32
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp1 live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : machine<enum-tag>
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : i32
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
"###);
}

#[test]
fn niche_detection_requires_option_isomorphic_pointer_shape() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let array_int = b.array("Array<Int>", INT);
    let option_array = b.option_enum("Option$Array$I", array_int);
    let option_i = b.option_enum("Option$I", INT);
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
                        ty: INT,
                    },
                    mir::Field {
                        name: "_2".to_string(),
                        ty: INT,
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
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Managed,
            payload_variant: 0,
        }
    ));
    assert!(matches!(
        edef(&module, option_array).repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Managed,
            payload_variant: 0,
        }
    ));
    assert!(matches!(
        edef(&module, flip).repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Managed,
            payload_variant: 1,
        }
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
            ty: lir::LirType::I32,
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
