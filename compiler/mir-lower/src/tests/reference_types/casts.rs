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
                                check_ty: s_ty,
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
                            check_ty: s_ty,
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
    check_mir_snapshot("is_instance_and_casts_lower_to_runtime_checks", &module);
}
