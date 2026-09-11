use super::*;

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
    let array_exact = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(array_class))
        .expect("Array<Int> has an exact identity")
        .identity_record()
        .id();
    let mutable_exact = source
        .meta
        .source_exact_types
        .get(&mir::Type::Class(mutable_class))
        .expect("MutableArray<Int> has an exact identity")
        .identity_record()
        .id();
    let module = lower(&source);

    assert_eq!(
        array_metadata(&module, "Array<Int>").identity,
        lir::LayoutIdentity::managed_array(array_exact, lir::LirTargetProfile::DARWIN_AARCH64,)
            .unwrap()
    );
    assert_eq!(
        array_metadata(&module, "MutableArray<Int>").identity,
        lir::LayoutIdentity::managed_array(mutable_exact, lir::LirTargetProfile::DARWIN_AARCH64,)
            .unwrap()
    );

    // Both nominal applications have managed-pointer storage, while every
    // instruction references its complete typed metadata record.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
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
  entry @scoop_main
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
    let module = lower(&b.finish(main));
    let dump = lir::dump(&module);
    assert!(dump.contains("array_assembly array0"), "{dump}");
    assert!(dump.contains("element integer<Int>(0x00000001)"), "{dump}");
    assert!(dump.contains("copy local0"), "{dump}");
}

#[test]
fn array_layouts_mark_reference_elements() {
    let mut b = Builder::new();
    let option_s = b.option_enum("Option$S", mir::Type::String);
    let point = b.strukt("Point", &[("x", INT), ("y", INT)]);
    let option_string = mir::Type::Enum(option_s, vec![mir::Type::String]);
    let array_int = b.array("Array<Int>", INT);
    let array_string = b.array("Array<String>", mir::Type::String);
    let array_option = b.array("Array<Option$S<String>>", option_string);
    let array_point = b.array("Array<Point>", mir::Type::Struct(point));
    let array_nested = b.array("Array<Array<Int>>", array_int.clone());
    let mut locals = Arena::new();
    let _ints = locals.alloc(local("ints", array_int.clone()));
    let _strings = locals.alloc(local("strings", array_string));
    let _options = locals.alloc(local("options", array_option));
    let _points = locals.alloc(local("points", array_point));
    let _nested = locals.alloc(local("nested", array_nested));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

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
        array_layout("Array<Option$S<String>>"),
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
    let module = lower(&b.finish(main));

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
