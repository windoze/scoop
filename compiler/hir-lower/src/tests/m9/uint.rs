use super::*;

// --- UInt ---

#[test]
fn uint_resolves_and_gcstats_returns_it() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "u",
            Some(ty_named("ULong")),
            call("gcStats", vec![]),
        )],
    )]);
    let module = lower_user_with_gc(file).expect("the UInt program must lower");
    assert_eq!(local_ty(&module, "u"), "ULong");
    // `gcStats` declares the `ULong` return type.
    let gc_stats = module
        .top_level
        .iter()
        .map(|&id| &module.functions[id])
        .find(|f| f.name == "gcStats")
        .expect("gcStats is declared in the GC core file");
    assert_eq!(
        module.types[gc_stats.return_ty],
        Type::Integer(hir::IntegerKind::UNSIGNED_64)
    );
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
        "integer literal `1` is not representable as UInt"
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
    assert_eq!(local_ty(&module, "sum"), "ULong");
    assert_eq!(local_ty(&module, "product"), "ULong");
    assert_eq!(local_ty(&module, "quotient"), "ULong");
    assert_eq!(local_ty(&module, "remainder"), "ULong");
    assert_eq!(local_ty(&module, "less"), "Boolean");
    assert_eq!(local_ty(&module, "same"), "Boolean");
    let hir::FunctionKind::User(body) = &module.functions[module.entry()].kind else {
        panic!("main body")
    };
    for (name, operation) in [
        ("sum", hir::NoGcIntegerOperation::Add),
        ("product", hir::NoGcIntegerOperation::Mul),
    ] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::NoGc {
                    kind: hir::IntegerKind::UNSIGNED_64,
                    operation: actual,
                    ..
                },
                ..
            } if *actual == operation
        ));
    }
    for (name, operation) in [
        ("quotient", hir::IntegerDivRem::Div),
        ("remainder", hir::IntegerDivRem::Rem),
    ] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::Managed {
                    kind: hir::IntegerKind::UNSIGNED_64,
                    operation: actual,
                    ..
                },
                ..
            } if *actual == operation
        ));
    }
    let hir::ExprKind::Binary { lhs, .. } = &local_init(body, "less").kind else {
        panic!("comparison must consume compareTo")
    };
    assert!(matches!(
        lhs.kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind: hir::IntegerKind::UNSIGNED_64,
                operation: hir::NoGcIntegerOperation::CompareTo,
                ..
            },
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
        "no applicable candidate for `plus` in member candidate layer:\n  - fun ULong.plus(other: ULong): ULong — argument for `other` (expected ULong): integer literal `1` is not representable as ULong; primitive integer operands require one exact type; convert this operand explicitly with `toUInt64()`"
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
        "no applicable candidate for `equals` in member candidate layer:\n  - fun ULong.equals(other: ULong): Boolean — argument for `other` has type Int, which is not a subtype of ULong; primitive integer operands require one exact type; convert this operand explicitly with `toUInt64()`"
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
    assert!(dump.contains("Call print<ULong> : Unit"), "{dump}");
    assert!(!dump.contains("Box : Any\n"), "{dump}");
}
