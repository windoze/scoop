use super::*;

#[test]
fn is_instance_and_casts_lower_to_runtime_checks() {
    let mut h = Harness::new();
    h.exception("ClassCastException");
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let option_s = h.option(s_ty);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let is_s = locals.alloc(local("is_s", boolean));
    let s2 = locals.alloc(local("s2", s_ty));
    let maybe = locals.alloc(local("maybe", option_s));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    is_s,
                    expr(
                        hir::ExprKind::IsInstance {
                            operand: Box::new(local_ref(a, any)),
                            check_ty: s_ty,
                        },
                        boolean,
                    ),
                ),
                val_decl(
                    s2,
                    // Mirror hir-lower's real shape: a value-typed
                    // `as` arrives as `Unbox(Cast)`; mir-lower's
                    // cast expansion only performs the check.
                    expr(
                        hir::ExprKind::Unbox(Box::new(expr(
                            hir::ExprKind::Cast {
                                operand: Box::new(local_ref(a, any)),
                                optional: false,
                            },
                            s_ty,
                        ))),
                        s_ty,
                    ),
                ),
                val_decl(
                    maybe,
                    expr(
                        hir::ExprKind::Cast {
                            operand: Box::new(local_ref(a, any)),
                            optional: true,
                        },
                        option_s,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // `is` stays a dedicated node; `as` throws
    // `ClassCastException` on failure (M8); `as?` wraps in
    // Some / None. The value-type checks registered the boxed
    // payload class. Capabilities are not synthesized from boxing.
    let expected = "\
Module
  struct S (x: Int)
  enum Option<S>
    Some(_1: S)
    None()
  class ClassCastException vtable=0 itables=0
  class box<S> vtable=0 itables=0
  generated_exact_type get0 location=class7 nominal_id=4bbfbdb3c9b6ef2334ee0604e763c6a09c3203bb80390c589a967b3dc705c883 exact_id=415a054b3cf2c6342b54e34b4072c957d9e02c1727e074fa28f0dadff613d4b8
  fun main @fn0() -> Unit
    bb0 entry
      val is_s: Boolean
        Type Boolean
        IsInstance S
          Type Any
          Local a
      val $cast.1: Any
        Type Any
        Local a
      branch bb1 bb2
        Type Boolean
        Unary BoolNot
          Type Boolean
          IsInstance S
            Type Any
            Local $cast.1
    bb1 if.then.1
      assign $new.1
        Type ClassCastException
        ClassAlloc ClassCastException
      call @fn1 direct
        Type ClassCastException
        Local $new.1
      throw
        Type ClassCastException
        Local $new.1
    bb2 if.merge.2
      val $ub.2: S
        Type S
        Unbox
          Type Any
          Local $cast.1
      val s2: S
        Type S
        Local $ub.2
      val $cast.3: Any
        Type Any
        Local a
      branch bb3 bb4
        Type Boolean
        IsInstance S
          Type Any
          Local $cast.3
    bb3 if.then.3
      assign $cast.4
        Type Option<S>
        VariantConstruct Option<S> v0
          Type S
          Unbox
            Type Any
            Local $cast.3
      goto bb5
    bb4 if.else.4
      assign $cast.4
        Type Option<S>
        VariantConstruct Option<S> v1
      goto bb5
    bb5 if.merge.5
      val maybe: Option<S>
        Type Option<S>
        Local $cast.4
      return
  fun init.ClassCastException.$c0 @fn1(this: ClassCastException) -> Unit
    bb0 entry
      return
  fun ctor.S.$c0 @fn7(x: Int) -> S <no-gc>
    bb0 entry
      return
        Type S
        StructInit S
          Type Int
          Local x
  output executable @fn0
";
    assert_eq!(dump(&module), expected);
}
