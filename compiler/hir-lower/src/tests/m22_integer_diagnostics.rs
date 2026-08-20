use super::*;

fn unsigned_integer(magnitude: u64) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix: ast::IntegerSuffix::Unsigned,
        span: sp(),
    })
}

#[test]
fn ambiguous_literal_fits_show_each_candidates_exact_commit() {
    let errors = lower_user(file(vec![
        fun_expr(
            "choose",
            Vec::new(),
            vec![("first", ty_named("Int8")), ("second", ty_named("Int16"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "choose",
            Vec::new(),
            vec![("first", ty_named("Int16")), ("second", ty_named("Int8"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun(
            "main",
            vec![stmt(call("choose", vec![int_lit(1), int_lit(2)]))],
        ),
    ]))
    .expect_err("multiple non-default literal fits must remain ambiguous");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "call to `choose` is ambiguous in current-unit top-level candidate layer:\n  - fun choose(first: Int8, second: Int16): Unit — tied after pairwise declaration forwarding; requires integer literal exact commits: argument for `first` = Int8, argument for `second` = Int16\n  - fun choose(first: Int16, second: Int8): Unit — tied after pairwise declaration forwarding; requires integer literal exact commits: argument for `first` = Int16, argument for `second` = Int8"
    );
}

#[test]
fn ambiguous_nested_literal_fits_show_the_committed_inner_type() {
    let errors = lower_user(file(vec![
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_nullable(ty_named("Int8")))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_nullable(ty_named("Int16")))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun("main", vec![stmt(call("select", vec![some(int_lit(1))]))]),
    ]))
    .expect_err("nested literal constraints remain candidate-local");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "call to `select` is ambiguous in current-unit top-level candidate layer:\n  - fun select(value: Option<Int8>): Unit — tied after pairwise declaration forwarding; requires integer literal exact commit: argument for `value` = Int8\n  - fun select(value: Option<Int16>): Unit — tied after pairwise declaration forwarding; requires integer literal exact commit: argument for `value` = Int16"
    );
}

#[test]
fn mixed_width_primitive_failure_suggests_the_receivers_exact_conversion() {
    let errors = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty("left", Some(ty_named("Int8")), int_lit(1)),
            val_ty("right", Some(ty_named("Int16")), int_lit(2)),
            val("result", binary(BinOp::Add, var("left"), var("right"))),
        ],
    )]))
    .expect_err("primitive integer arithmetic does not implicitly change width");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `plus` in member candidate layer:\n  - fun Int8.plus(other: Int8): Int8 — argument for `other` has type Int16, which is not a subtype of Int8; primitive integer operands require one exact type; convert this operand explicitly with `toInt8()`"
    );
}

#[test]
fn mixed_signedness_literal_failure_suggests_an_explicit_conversion() {
    let errors = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty("left", Some(ty_named("UInt8")), unsigned_integer(1)),
            val("result", binary(BinOp::Add, var("left"), int_lit(2))),
        ],
    )]))
    .expect_err("primitive integer arithmetic does not implicitly change signedness");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `plus` in member candidate layer:\n  - fun UInt8.plus(other: UInt8): UInt8 — argument for `other` (expected UInt8): integer literal `2` is not representable as UInt8; primitive integer operands require one exact type; convert this operand explicitly with `toUInt8()`"
    );
}
