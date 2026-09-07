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
Module mangling=persistent-v1
  struct S (x: Int)
  enum Option$D1_SX
    Some(_1: S)
    None()
  class ClassCastException vtable=0 itables=0
  class box$D1_SX vtable=0 itables=0
  fun main @scoop_main() -> Unit
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
      call @scoop.init.ClassCastException.$c0 direct
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
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v0
          Type S
          Unbox
            Type Any
            Local $cast.3
      goto bb5
    bb4 if.else.4
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v1
      goto bb5
    bb5 if.merge.5
      val maybe: Option$D1_SX<S>
        Type Option$D1_SX<S>
        Local $cast.4
      return
  fun init.ClassCastException.$c0 @scoop.init.ClassCastException.$c0(this: ClassCastException) -> Unit
    bb0 entry
      return
  fun ctor.S.$c0 @scoop.ctor.S.$c0(x: Int) -> S <no-gc>
    bb0 entry
      return
        Type S
        StructInit S
          Type Int
          Local x
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}
