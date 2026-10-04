use super::*;

#[test]
fn generic_array_preserves_application_shape_and_allocation() {
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", INT);
    let mir::Type::Class(array_class) = array_int else {
        unreachable!("Array<Int> is a class application")
    };
    let mut locals = Arena::new();
    let values = locals.alloc(local("values", mir::Type::Class(array_class)));
    let main = b.main(
        locals,
        vec![val_decl(
            values,
            expr(
                mir::Type::Class(array_class),
                mir::ExprKind::ArrayLiteral {
                    array_type: array_class,
                    elements: vec![int_expr(1)],
                },
            ),
        )],
    );
    let mut source = b.finish(main);
    mark_test_nominal_application(&mut source, mir::Type::Class(array_class));

    let identity = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(array_class))
        .unwrap();
    let exact = identity.identity_record().id();
    let group = identity.nominal_specialization().unwrap().id();
    let output = try_lower(source).expect("generic array shapes use their source application");
    let module = output.module();
    assert_eq!(
        array_metadata(module, "Array<Int>").identity,
        lir::LayoutIdentity::managed_array(
            exact,
            module.meta.target_profile,
            lir::MaterializationRoot::prior_stage_odr(group),
        )
        .unwrap()
    );
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find(|(_, descriptor)| descriptor.identity.exact_type() == exact)
        .unwrap()
        .1;
    assert_eq!(
        descriptor.identity.symbol_request().linkage(),
        scoop_identity::LinkageClass::OdrWeak
    );
    let immortals = lir::StrongImmortalObjectSemanticPlanSetV1::from_module(module).unwrap();
    let storages = lir::StrongStaticStorageSemanticPlanSetV1::from_module(module).unwrap();
    let shapes = lir::CanonicalShapeLirDefinitionsV1::from_module(
        module,
        output.foundation(),
        immortals.objects().iter().copied(),
        storages.storages(),
    )
    .expect("array metadata supplies its complete physical shape content");
    assert_eq!(shapes.definitions().len(), 7);
    for (role, count) in [
        (scoop_identity::OdrMemberRole::Layout, 2),
        (scoop_identity::OdrMemberRole::ScanProgram, 3),
        (scoop_identity::OdrMemberRole::TypeDescriptor, 1),
        (scoop_identity::OdrMemberRole::DispatchTable, 1),
    ] {
        assert_eq!(
            shapes
                .definitions()
                .iter()
                .filter(|definition| definition.role() == role)
                .count(),
            count,
        );
    }
    assert!(
        shapes
            .definitions()
            .iter()
            .all(|definition| definition.group() == group)
    );
    assert!(lir::dump(module).contains("array_alloc array0 (integer<Int>(0x00000001))"));
}

#[test]
fn array_nodes_become_array_instructions() {
    // val a = [1, 2]; val x = a[0]; val n = a.size
    // val m = MutableArray(a); m[0] = 40
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", INT);
    let mutable_int = b.mutable_array("MutableArray<Int>", INT);
    let mir::Type::Class(array_class) = array_int else {
        unreachable!()
    };
    let mir::Type::Class(mutable_class) = mutable_int else {
        unreachable!()
    };
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", mir::Type::Class(array_class)));
    let x = locals.alloc(local("x", INT));
    let n = locals.alloc(local("n", LONG));
    let m = locals.alloc(local("m", mir::Type::Class(mutable_class)));
    let main = b.main(
        locals,
        vec![
            val_decl(
                a,
                expr(
                    mir::Type::Class(array_class),
                    mir::ExprKind::ArrayLiteral {
                        array_type: array_class,
                        elements: vec![int_expr(1), int_expr(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    INT,
                    mir::ExprKind::ArrayGet {
                        array_type: array_class,
                        array: Box::new(local_expr(a, mir::Type::Class(array_class))),
                        index: Box::new(long_expr(0)),
                    },
                ),
            ),
            val_decl(
                n,
                expr(
                    LONG,
                    mir::ExprKind::ArrayLen {
                        array_type: array_class,
                        operand: Box::new(local_expr(a, mir::Type::Class(array_class))),
                    },
                ),
            ),
            val_decl(
                m,
                expr(
                    mir::Type::Class(mutable_class),
                    mir::ExprKind::ArrayClone {
                        source_type: array_class,
                        target_type: mutable_class,
                        operand: Box::new(local_expr(a, mir::Type::Class(array_class))),
                    },
                ),
            ),
            stmt(mir::StatementKind::ArraySet {
                array_type: mutable_class,
                array: local_expr(m, mir::Type::Class(mutable_class)),
                index: long_expr(0),
                value: int_expr(40),
            }),
        ],
    );
    let source = b.finish(main);
    let array_identity = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(array_class))
        .expect("Array<Int> has an exact identity");
    let array_exact = array_identity.identity_record().id();
    assert_eq!(
        array_identity.owner(),
        mir::SourceExactTypeOwner::Cone(source.cone)
    );
    let mutable_identity = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(mutable_class))
        .expect("MutableArray<Int> has an exact identity");
    let mutable_exact = mutable_identity.identity_record().id();
    assert_eq!(
        mutable_identity.owner(),
        mir::SourceExactTypeOwner::Cone(source.cone)
    );
    let module = lower(source);

    assert_eq!(
        array_metadata(&module, "Array<Int>").identity,
        lir::LayoutIdentity::managed_array(
            array_exact,
            lir::LirTargetProfile::DARWIN_AARCH64,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
    assert_eq!(
        array_metadata(&module, "MutableArray<Int>").identity,
        lir::LayoutIdentity::managed_array(
            mutable_exact,
            lir::LirTargetProfile::DARWIN_AARCH64,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );

    // Both nominal applications have managed-pointer storage, while every
    // instruction references its complete typed metadata record.
    insta::assert_snapshot!(lir::dump(&module), @r#"
    Module
      global @scoop$1$ss$9b273ab0bbc562dd7f8e8b0487c0e98f4a7d0781b1cb5aa5b6d69c2d8a7f66b1 : ptr<managed> scan=refs[0]
      fun @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca() -> void
        local %0 a: ptr<managed>
        local %1 x: i32
        local %2 n: i64
        local %3 m: ptr<managed>
      block entry
        poll managed-void-target0 sp<managed-poll:0> live=[]
        t0 = array_alloc array0 (integer<Int>(0x00000001), integer<Int>(0x00000002)) sp<managed-call:0> live  : ptr<managed>
        store t0 -> local0
        t1 = array_get array0 local0 integer<Long>(0x0000000000000000) : i32
        store t1 -> local1
        t2 = array_len array0 local0 : i64
        store t2 -> local2
        t3 = array_clone array0 -> array1 local0 sp<managed-call:1> live local0:ptr<managed>@0 : ptr<managed>
        store t3 -> local3
        array_set array1 local3 integer<Long>(0x0000000000000000) integer<Int>(0x00000028)
        ret
      fun @scoop$1$cb$d3bd523ea7c4b775508c06e622f76772db6a21fddb406c6d3fe7d1f20a2a89c1() -> i32
      block entry
        poll managed-void-target1 sp<managed-poll:0> live=[]
        call managed-direct-target1 sp<managed-call:0> live=[] t4 = sig=direct1 (ptr<metadata>) -> ptr<managed> runtime @scoop_rt_context_ensure_root(td12)
        invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn0() normal @success unwind @failure
        br @success
      block success
        ret integer<UInt>(0x00000000)
      block failure
        (t0, t1) = landingpad : (exception_record, ptr<raw>)
        t2 = begin_catch t1 : ptr<managed>
        call managed-direct-target0 sp<managed-call:1> live=[t2:ptr<managed>@0] t3 = sig=direct0 (ptr<managed>) -> ptr<managed> runtime @scoop_rt_materialize_exception(t2)
        global_store global0, t3
        end_catch
        ret integer<UInt>(0x00000001)
      td td3 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td4 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td5 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td6 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td7 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td8 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td9 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td10 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td11 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td12 task-context @scoop$1$td$db9fdace23f2040d3622172122120f4e46c6786180d6495caf23e609df021eac type-id=11462109518987149384 shape=FixedObject minimum-size=24 align=8 parent=none vtable=[] itables=[]
      array-type array0 Array<Int> kind=immutable element=i32 size=4 align=4 scan=none td=td0
      array-type array1 MutableArray<Int> kind=mutable element=i32 size=4 align=4 scan=none td=td1
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
      layout task-context value size=8 align=8 refs=[0]
      layout task-context size=24 align=8 refs=[16]
      layout Array<Int> value size=8 align=8 refs=[0]
      layout MutableArray<Int> value size=8 align=8 refs=[0]
      layout String value size=8 align=8 refs=[0]
      output executable @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca
    "#);
}

#[test]
fn array_assembly_becomes_one_typed_allocation_instruction() {
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", INT);
    let mir::Type::Class(array_class) = array_int else {
        unreachable!("Array<Int> is a class application")
    };
    let mut locals = Arena::new();
    let source = locals.alloc(local("source", mir::Type::Class(array_class)));
    let assembled = locals.alloc(local("assembled", mir::Type::Class(array_class)));
    let main = b.main(
        locals,
        vec![
            val_decl(
                source,
                expr(
                    mir::Type::Class(array_class),
                    mir::ExprKind::ArrayLiteral {
                        array_type: array_class,
                        elements: vec![int_expr(2)],
                    },
                ),
            ),
            val_decl(
                assembled,
                expr(
                    mir::Type::Class(array_class),
                    mir::ExprKind::ArrayAssembly {
                        array_type: array_class,
                        parts: vec![
                            mir::ArrayAssemblyPart::Element(int_expr(1)),
                            mir::ArrayAssemblyPart::CopyArray(local_expr(
                                source,
                                mir::Type::Class(array_class),
                            )),
                        ],
                    },
                ),
            ),
        ],
    );
    let module = lower(b.finish(main));
    let dump = lir::dump(&module);
    assert!(dump.contains("array_assembly array0"), "{dump}");
    assert!(dump.contains("element integer<Int>(0x00000001)"), "{dump}");
    assert!(dump.contains("copy local0"), "{dump}");
}

#[test]
fn array_layouts_mark_reference_elements() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option<String>", mir::Type::String);
    let point = b.strukt("Point", &[("x", INT), ("y", INT)]);
    let option_string = mir::Type::Enum(option_s, vec![mir::Type::String]);
    let array_int = b.array("Array<Int>", INT);
    let array_string = b.array("Array<String>", mir::Type::String);
    let array_option = b.array("Array<Option<String>>", option_string);
    let array_point = b.array("Array<Point>", mir::Type::Struct(point));
    let array_nested = b.array("Array<Array<Int>>", array_int.clone());
    let mut locals = Arena::new();
    let _ints = locals.alloc(local("ints", array_int.clone()));
    let _strings = locals.alloc(local("strings", array_string));
    let _options = locals.alloc(local("options", array_option));
    let _points = locals.alloc(local("points", array_point));
    let _nested = locals.alloc(local("nested", array_nested));
    let main = b.main(locals, vec![]);
    let module = lower(b.finish(main));

    let array_layout = |name: &str| {
        let array = array_metadata(&module, name);
        assert_eq!(
            *array.layout.instance().inline_scan(),
            *array_scan(descriptor(&module, array.type_descriptor)),
            "array metadata and its descriptor must carry one closed element scan"
        );
        (
            array.layout.instance().inline_size(),
            array.layout.instance().inline_alignment(),
            array.layout.instance().inline_scan().clone(),
        )
    };
    // size / align are element-level: the element stride and
    // alignment of the region after header + size.
    assert_eq!(array_layout("Array<Int>"), (4, 4, lir::RefScan::None));
    // String elements are references.
    assert_eq!(
        array_layout("Array<String>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
    // Option<String> uses the niche representation — a bare
    // pointer, hence a reference element.
    assert_eq!(
        array_layout("Array<Option<String>>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
    // A value-type element is inline: the Point stride.
    assert_eq!(array_layout("Array<Point>"), (8, 4, lir::RefScan::None));
    // An array element is itself a reference; the nested element
    // type gets its own layout too.
    assert_eq!(
        array_layout("Array<Array<Int>>"),
        (8, 8, lir::RefScan::References(vec![0]))
    );
}

#[test]
fn array_fields_are_reference_fields() {
    let mut b = Builder::new();
    let array_int = b.array("Array<Int>", INT);
    let _holder = b.strukt("Holder", &[("flag", mir::Type::Boolean), ("xs", array_int)]);
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));

    // flag @0 (1 byte), xs @8: an array value is a pointer-sized
    // reference.
    let holder = layout_values(&module)
        .find(|l| l.name == "Holder")
        .expect("a layout per struct");
    assert_eq!((holder.size, holder.align), (16, 8));
    assert_eq!(plain_refs(holder), [8]);
    // The complete intrinsic class application exists independently of
    // whether a function contains an array instruction.
    assert_eq!(
        array_metadata(&module, "Array<Int>").element,
        lir::LirType::I32
    );
}
