use super::super::*;

#[test]
fn array_nodes_translate_one_to_one() {
    // val a = [1, 2, 3]; val x = a[0]; val n = a.size
    // val m: MutableArray<Int> = MutableArray(a); m[0] = 40
    let mut h = Harness::new();
    h.exception("IndexOutOfBoundsException");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", array_int));
    let x = locals.alloc(local("x", int));
    let n = locals.alloc(local("n", h.long));
    let m = locals.alloc(local("m", mutable_int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    a,
                    expr(
                        hir::ExprKind::ArrayLiteral(vec![
                            int_lit(&h, 1),
                            int_lit(&h, 2),
                            int_lit(&h, 3),
                        ]),
                        array_int,
                    ),
                ),
                val_decl(
                    x,
                    expr(
                        hir::ExprKind::Index {
                            access: hir::ArrayAccessKind::ImmutableGet,
                            receiver: Box::new(local_ref(a, array_int)),
                            index: Box::new(integer_lit(&h, hir::IntegerKind::SIGNED_64, 0)),
                        },
                        int,
                    ),
                ),
                val_decl(
                    n,
                    expr(
                        hir::ExprKind::ArrayLen(Box::new(local_ref(a, array_int))),
                        h.long,
                    ),
                ),
                val_decl(
                    m,
                    expr(
                        hir::ExprKind::ArrayClone(Box::new(local_ref(a, array_int))),
                        mutable_int,
                    ),
                ),
                stmt(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Index {
                        array: Box::new(local_ref(m, mutable_int)),
                        index: Box::new(integer_lit(&h, hir::IntegerKind::SIGNED_64, 0)),
                    },
                    value: int_lit(&h, 40),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subscript read and the indexed store both get the M8
    // bounds check: array and index evaluated once into hidden
    // locals, then `IndexOutOfBoundsException` on failure.
    let expected = "\
Module
  class IndexOutOfBoundsException vtable=0 itables=0
  fun main @fn0() -> Unit
    bb0 entry
      val a: Array<Int>
        Type Array<Int>
        ArrayLiteral Array<Int>
          Type Int
          IntegerLiteral Int value=1 bits=0x00000001
          Type Int
          IntegerLiteral Int value=2 bits=0x00000002
          Type Int
          IntegerLiteral Int value=3 bits=0x00000003
      val $arr.1: Array<Int>
        Type Array<Int>
        Local a
      val $idx.2: Long
        Type Long
        IntegerLiteral Long value=0 bits=0x0000000000000000
      branch bb2 bb1
        Type Boolean
        IntegerCompare less-than operands=Long result=Boolean
          Type Long
          Local $idx.2
          Type Long
          IntegerLiteral Long value=0 bits=0x0000000000000000
    bb1 logic.rhs.1
      assign $logic.1
        Type Boolean
        IntegerCompare greater-than-or-equal operands=Long result=Boolean
          Type Long
          Local $idx.2
          Type Long
          ArrayLen Array<Int>
            Type Array<Int>
            Local $arr.1
      goto bb3
    bb2 logic.short.2
      assign $logic.1
        Type Boolean
        BoolLiteral true
      goto bb3
    bb3 logic.merge.3
      branch bb4 bb5
        Type Boolean
        Local $logic.1
    bb4 if.then.4
      assign $new.2
        Type IndexOutOfBoundsException
        ClassAlloc IndexOutOfBoundsException
      call @fn1 direct
        Type IndexOutOfBoundsException
        Local $new.2
      throw
        Type IndexOutOfBoundsException
        Local $new.2
    bb5 if.merge.5
      val x: Int
        Type Int
        ArrayGet Array<Int>
          Type Array<Int>
          Local $arr.1
          Type Long
          Local $idx.2
      val n: Long
        Type Long
        ArrayLen Array<Int>
          Type Array<Int>
          Local a
      val m: MutableArray<Int>
        Type MutableArray<Int>
        ArrayClone Array<Int> -> MutableArray<Int>
          Type Array<Int>
          Local a
      val $arr.3: MutableArray<Int>
        Type MutableArray<Int>
        Local m
      val $idx.4: Long
        Type Long
        IntegerLiteral Long value=0 bits=0x0000000000000000
      branch bb7 bb6
        Type Boolean
        IntegerCompare less-than operands=Long result=Boolean
          Type Long
          Local $idx.4
          Type Long
          IntegerLiteral Long value=0 bits=0x0000000000000000
    bb6 logic.rhs.6
      assign $logic.3
        Type Boolean
        IntegerCompare greater-than-or-equal operands=Long result=Boolean
          Type Long
          Local $idx.4
          Type Long
          ArrayLen MutableArray<Int>
            Type MutableArray<Int>
            Local $arr.3
      goto bb8
    bb7 logic.short.7
      assign $logic.3
        Type Boolean
        BoolLiteral true
      goto bb8
    bb8 logic.merge.8
      branch bb9 bb10
        Type Boolean
        Local $logic.3
    bb9 if.then.9
      assign $new.4
        Type IndexOutOfBoundsException
        ClassAlloc IndexOutOfBoundsException
      call @fn1 direct
        Type IndexOutOfBoundsException
        Local $new.4
      throw
        Type IndexOutOfBoundsException
        Local $new.4
    bb10 if.merge.10
      array_set MutableArray<Int>
        Type MutableArray<Int>
        Local $arr.3
        Type Long
        Local $idx.4
        Type Int
        IntegerLiteral Int value=40 bits=0x00000028
      return
  fun init.IndexOutOfBoundsException.$c0 @fn1(this: IndexOutOfBoundsException) -> Unit
    bb0 entry
      return
  output executable @fn0
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn array_assembly_preserves_typed_element_and_copy_parts() {
    let mut h = Harness::new();
    let int = h.int;
    let array_int = h.array(int);
    let hir::Type::Class(array_application) = h.types[array_int] else {
        unreachable!("Array<Int> has a class application")
    };
    let mut locals = Arena::new();
    let source = locals.alloc(local("source", array_int));
    let assembled = locals.alloc(local("assembled", array_int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    source,
                    expr(hir::ExprKind::ArrayLiteral(vec![int_lit(&h, 2)]), array_int),
                ),
                val_decl(
                    assembled,
                    expr(
                        hir::ExprKind::ArrayAssembly(hir::ArrayAssembly {
                            element_type: int,
                            parts: vec![
                                hir::ArrayAssemblyPart::Element(int_lit(&h, 1)),
                                hir::ArrayAssemblyPart::CopyArray(local_ref(source, array_int)),
                            ],
                            result_type: array_application,
                        }),
                        array_int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let dump = dump(&module);
    assert!(dump.contains("ArrayAssembly Array<Int>"), "{dump}");
    assert!(dump.contains("Element\n"), "{dump}");
    assert!(dump.contains("CopyArray\n"), "{dump}");
}

#[test]
fn instance_types_preserve_array_arguments() {
    let mut h = Harness::new();
    let f = identity_fn(&mut h, "f");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.use_identity_instances(f, &[array_int, mutable_int]);
    let module = lower(&h.finish(main));

    assert_eq!(module.meta.instances.len(), 2);
    // Substitution recurses into the array element types.
    let instances = module
        .functions
        .iter()
        .filter_map(|(_, function)| (function.name == "f").then_some(function))
        .collect::<Vec<_>>();
    let array_instance = instances[0];
    assert_eq!(
        mir::array_type(&module, &array_instance.params[0].ty),
        Some((
            mir::ArrayKind::Immutable,
            &mir::Type::Integer(mir::IntegerKind::SIGNED_32)
        ))
    );
    let mutable_instance = instances[1];
    assert_eq!(
        mir::array_type(&module, &mutable_instance.return_ty),
        Some((
            mir::ArrayKind::Mutable,
            &mir::Type::Integer(mir::IntegerKind::SIGNED_32)
        ))
    );
}
