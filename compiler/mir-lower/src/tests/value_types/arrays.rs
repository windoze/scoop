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
    let n = locals.alloc(local("n", int));
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
                            receiver: Box::new(local_ref(a, array_int)),
                            index: Box::new(int_lit(&h, 0)),
                        },
                        int,
                    ),
                ),
                val_decl(
                    n,
                    expr(
                        hir::ExprKind::ArrayLen(Box::new(local_ref(a, array_int))),
                        int,
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
                        array: local_ref(m, mutable_int),
                        index: int_lit(&h, 0),
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
  fun main @scoop_main() -> Unit
    bb0 entry
      val a: Array<Int>
        Type Array<Int>
        ArrayLiteral Array$I
          Type Int
          IntLiteral 1
          Type Int
          IntLiteral 2
          Type Int
          IntLiteral 3
      val $arr.1: Array<Int>
        Type Array<Int>
        Local a
      val $idx.2: Int
        Type Int
        IntLiteral 0
      branch bb2 bb1
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.2
          Type Int
          IntLiteral 0
    bb1 logic.rhs.1
      assign $logic.1
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.2
          Type Int
          ArrayLen Array$I
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
      call $call.2: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.2
    bb5 if.merge.5
      val x: Int
        Type Int
        ArrayGet Array$I
          Type Array<Int>
          Local $arr.1
          Type Int
          Local $idx.2
      val n: Int
        Type Int
        ArrayLen Array$I
          Type Array<Int>
          Local a
      val m: MutableArray<Int>
        Type MutableArray<Int>
        ArrayClone Array$I -> MutableArray$I
          Type Array<Int>
          Local a
      val $arr.3: MutableArray<Int>
        Type MutableArray<Int>
        Local m
      val $idx.4: Int
        Type Int
        IntLiteral 0
      branch bb7 bb6
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.4
          Type Int
          IntLiteral 0
    bb6 logic.rhs.6
      assign $logic.3
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.4
          Type Int
          ArrayLen MutableArray$I
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
      call $call.4: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.4
    bb10 if.merge.10
      array_set MutableArray$I
        Type MutableArray<Int>
        Local $arr.3
        Type Int
        Local $idx.4
        Type Int
        IntLiteral 40
      return
  fun ctor.IndexOutOfBoundsException @scoop.ctor.IndexOutOfBoundsException() -> IndexOutOfBoundsException
    bb0 entry
      return
        Type IndexOutOfBoundsException
        ClassInit IndexOutOfBoundsException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn instance_symbols_encode_array_arguments() {
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
    h.instantiate(f, vec![array_int]);
    h.instantiate(f, vec![mutable_int]);
    let module = lower(&h.finish(main));

    let symbols: Vec<&str> = module.top_level[1..]
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    // `mir::encode_type`: `A<element>X` / `M<element>X`.
    assert_eq!(symbols, ["scoop.f$AIX", "scoop.f$MIX"]);
    // Substitution recurses into the array element types.
    let array_instance = &module.functions[module.top_level[1]];
    assert_eq!(
        mir::array_type(&module, &array_instance.params[0].ty),
        Some((mir::ArrayKind::Immutable, &mir::Type::Int))
    );
    let mutable_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        mir::array_type(&module, &mutable_instance.return_ty),
        Some((mir::ArrayKind::Mutable, &mir::Type::Int))
    );
}
