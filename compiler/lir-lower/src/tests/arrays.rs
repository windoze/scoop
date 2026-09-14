use super::*;

#[test]
fn reachable_generic_array_reports_stable_strong_capability_error() {
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

    let error = match try_lower(source) {
        Err(error) => error,
        Ok(_) => panic!("reachable generic array must fail strong LIR capability validation"),
    };
    assert!(
        error
            .to_string()
            .starts_with(StrongLirCapabilityError::CODE)
    );
    match error {
        StrongLirLoweringError::Capability(error) => {
            assert_eq!(error.function(), main);
            assert_eq!(
                error.requirement(),
                &StrongLirMaterializationRequirement::ArrayType(array_class)
            );
        }
        StrongLirLoweringError::Foundation(_) => {
            panic!("capability validation must run before LIR foundation projection")
        }
    }
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
    assert_eq!(array_identity.owner(), mir::SourceExactTypeOwner::ConeOwned);
    let mutable_identity = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(mutable_class))
        .expect("MutableArray<Int> has an exact identity");
    let mutable_exact = mutable_identity.identity_record().id();
    assert_eq!(
        mutable_identity.owner(),
        mir::SourceExactTypeOwner::ConeOwned
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
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
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
    t3 = array_clone array1 local0 sp<managed-call:1> live local0:ptr<managed>@0 : ptr<managed>
    store t3 -> local3
    array_set array1 local3 integer<Long>(0x0000000000000000) integer<Int>(0x00000028)
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
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
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
        (
            array.element_size,
            array.element_align,
            array_scan(descriptor(&module, array.type_descriptor)).clone(),
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
