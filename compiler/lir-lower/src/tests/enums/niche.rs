//! Option-isomorphic niche eligibility and representation.

use super::*;

#[test]
fn option_of_string_uses_the_niche_representation() {
    // Option<String>: the payload maps to `Ptr`, so the value is
    // the pointer itself with None = null (spec 7.4).
    let module = lower(option_round_trip("Option<String>", mir::Type::String));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  enum Option<String> niche(kind=managed,payload_variant=0)
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 o: enum0
    local %1 t: machine<enum-tag>
    local %2 p: ptr<managed>
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : machine<enum-tag>
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr<managed>
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn0() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Option<String><String> @scoop$1$td$788cbf7a74af24a4ac1fd29f1cc8c147f7003a53cef568d986886a74b83631a7 type-id=4844072839988068642 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
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
  layout Option<String> size=8 align=8 enum-scan=refs[0]
  layout String value size=8 align=8 refs=[0]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn option_of_raw_pointer_uses_a_niche_without_gc_scanning() {
    let mut builder = Builder::new();
    let option = builder.option_enum("Option<Ptr<Int>>", mir::Type::Ptr(Box::new(INT)));
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(builder.finish(main));

    assert!(matches!(
        edef(&module, option).repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Raw,
            payload_variant: 0,
        }
    ));
    let layout = layout_values(&module)
        .find(|layout| layout.name == "Option<Ptr<Int>>")
        .expect("raw pointer option layout");
    let lir::LayoutKind::Enum { scan } = &layout.kind else {
        panic!("Option<Ptr<Int>> must retain its enum layout identity")
    };
    assert_eq!(*scan, lir::RefScan::None);
}

#[test]
fn option_of_code_pointer_records_code_niche_provenance() {
    let mut builder = Builder::new();
    let signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let option = builder.enums.alloc(mir::EnumDef {
        name: "Option<FunPtr>".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            test_variant(
                "Some".to_string(),
                true,
                vec![mir::Field {
                    name: "_1".to_string(),
                    ty: mir::Type::FunPtr(signature),
                }],
            ),
            test_variant("None".to_string(), true, Vec::new()),
        ],
    });
    let main = builder.main(Arena::new(), Vec::new());
    let mir_module = builder.finish(main);

    let module = lower(mir_module);

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
    let module = lower(option_round_trip("Option<Int>", INT));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  enum Option<Int> tagged size=16 align=8 variants=(i32)@8+4 ()@8+0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 o: enum0
    local %1 t: machine<enum-tag>
    local %2 p: i32
    local %3 o2: enum0
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : machine<enum-tag>
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : i32
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn0() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Option<Int><Int> @scoop$1$td$788cbf7a74af24a4ac1fd29f1cc8c147f7003a53cef568d986886a74b83631a7 type-id=4844072839988068642 shape=BoxedValue minimum-size=32 align=8 parent=none vtable=[] itables=[]
  td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
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
  layout Option<Int> size=16 align=8 enum-scan=none
  layout String value size=8 align=8 refs=[0]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn niche_detection_requires_option_isomorphic_pointer_shape() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option<String>", mir::Type::String);
    let array_int = b.array("Array<Int>", INT);
    let option_array = b.option_enum("Option<Array<Int>>", array_int);
    let option_i = b.option_enum("Option<Int>", INT);
    // Reversed declaration order: the payload variant comes second.
    let flip = b.enums.alloc(mir::EnumDef {
        name: "Flip".to_string(),
        type_arguments: Vec::new(),
        gc_free: false,
        variants: vec![
            test_variant("Naught".to_string(), true, Vec::new()),
            test_variant(
                "Value".to_string(),
                false,
                vec![mir::Field {
                    name: "_1".to_string(),
                    ty: mir::Type::String,
                }],
            ),
        ],
    });
    // Two variants, but the payload variant has two fields: tagged.
    let pair_or_none = b.enums.alloc(mir::EnumDef {
        name: "PairOrNone".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            test_variant(
                "Pair".to_string(),
                true,
                vec![
                    mir::Field {
                        name: "_1".to_string(),
                        ty: INT,
                    },
                    mir::Field {
                        name: "_2".to_string(),
                        ty: INT,
                    },
                ],
            ),
            test_variant("Empty".to_string(), true, Vec::new()),
        ],
    });
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(b.finish(main));

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
