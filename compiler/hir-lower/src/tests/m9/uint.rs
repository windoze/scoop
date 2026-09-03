use super::*;

// --- UInt ---

#[test]
fn uint_resolves_and_gcstats_returns_it() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("u", Some(ty_named("UInt")), call("gcStats", vec![]))],
    )]);
    let module = lower_user_with_gc(file).expect("the UInt program must lower");
    assert_eq!(local_ty(&module, "u"), "UInt");
    // `gcStats` declares the `UInt` return type.
    let gc_stats = module
        .top_level
        .iter()
        .map(|&id| &module.functions[id])
        .find(|f| f.name == "gcStats")
        .expect("gcStats is declared in the GC core file");
    assert_eq!(module.types[gc_stats.return_ty], Type::UInt);
}

#[test]
fn uint_and_int_are_different_types() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("u", Some(ty_named("UInt")), int_lit(1))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("an Int literal is no UInt");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `u` must be of type UInt, found Int"
    );
}

#[test]
fn uint_arithmetic_comparison_and_equality() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", call("gcStats", vec![])),
            val("b", call("gcStats", vec![])),
            val("sum", binary(BinOp::Add, var("a"), var("b"))),
            val("product", binary(BinOp::Mul, var("a"), var("b"))),
            val("quotient", binary(BinOp::Div, var("a"), var("b"))),
            val("remainder", binary(BinOp::Rem, var("a"), var("b"))),
            val("less", binary(BinOp::Lt, var("a"), var("b"))),
            val("same", binary(BinOp::Eq, var("a"), var("b"))),
        ],
    )]);
    let module = lower_user_with_gc(file).expect("UInt arithmetic must lower");
    assert_eq!(local_ty(&module, "sum"), "UInt");
    assert_eq!(local_ty(&module, "product"), "UInt");
    assert_eq!(local_ty(&module, "quotient"), "UInt");
    assert_eq!(local_ty(&module, "remainder"), "UInt");
    assert_eq!(local_ty(&module, "less"), "Boolean");
    assert_eq!(local_ty(&module, "same"), "Boolean");
    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    for (name, kind) in [
        ("sum", hir::PrimitiveBinaryKind::UIntAdd),
        ("product", hir::PrimitiveBinaryKind::UIntMul),
        ("quotient", hir::PrimitiveBinaryKind::UIntDiv),
        ("remainder", hir::PrimitiveBinaryKind::UIntRem),
    ] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::PrimitiveBinary { kind: actual, .. } if *actual == kind
        ));
    }
    let hir::ExprKind::Binary { lhs, .. } = &local_init(body, "less").kind else {
        panic!("comparison must consume compareTo")
    };
    assert!(matches!(
        lhs.kind,
        hir::ExprKind::PrimitiveBinary {
            kind: hir::PrimitiveBinaryKind::UIntCompareTo,
            ..
        }
    ));
}

#[test]
fn uint_mixed_arithmetic_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val(
            "x",
            binary(BinOp::Add, call("gcStats", vec![]), int_lit(1)),
        )],
    )]);
    let errors = lower_user_with_gc(file).expect_err("Int and UInt do not mix");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `plus` in member candidate layer:\n  - fun UInt.plus(other: UInt): UInt — argument for `other` has type Int, which is not a subtype of UInt"
    );
}

#[test]
fn uint_mixed_equality_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val(
            "x",
            binary(BinOp::Eq, call("gcStats", vec![]), int_lit(1)),
        )],
    )]);
    let errors = lower_user_with_gc(file).expect_err("UInt and Int compare unequal");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `equals` in member candidate layer:\n  - fun UInt.equals(other: UInt): Boolean — argument for `other` has type Int, which is not a subtype of UInt"
    );
}

#[test]
fn uint_print_uses_the_to_string_instantiation_without_boxing() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call("print", vec![call("gcStats", vec![])]))],
    )]);
    let module = lower_user_with_gc(file).expect("printing a UInt must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("Call print<UInt> : Unit"), "{dump}");
    assert!(!dump.contains("Box : Any\n"), "{dump}");
}
