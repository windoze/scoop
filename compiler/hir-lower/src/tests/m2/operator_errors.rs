use super::super::*;

// --- negative: operators ---

#[test]
fn arithmetic_requires_int_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Add, int_lit(1), str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int arithmetic must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `+` requires Int operands, found Int and String"
    );
}

#[test]
fn comparison_requires_int_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Lt, str_lit("a"), str_lit("b")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int comparison must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `<` requires Int operands, found String and String"
    );
}

#[test]
fn equality_requires_matching_types() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Eq, int_lit(1), str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("mismatched equality must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `equals` in member candidate layer:\n  - fun Int.equals(other: Int): Boolean — argument for `other` has type String, which is not a subtype of Int"
    );
}

#[test]
fn logical_and_requires_boolean_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::And, int_lit(1), bool_lit(true)))],
    )]);
    let errors = lower_user(file).expect_err("non-Boolean `&&` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `&&` requires Boolean operands, found Int and Boolean"
    );
}

#[test]
fn negation_requires_an_int_operand() {
    let file = file(vec![fun(
        "main",
        vec![val("x", unary(UnOp::Neg, str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int negation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `-` requires an Int operand, found String"
    );
}

#[test]
fn logical_not_requires_a_boolean_operand() {
    let file = file(vec![fun(
        "main",
        vec![val("x", unary(UnOp::Not, int_lit(1)))],
    )]);
    let errors = lower_user(file).expect_err("non-Boolean `!` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `!` requires a Boolean operand, found Int"
    );
}

// --- negative: conditions ---

#[test]
fn if_condition_must_be_boolean() {
    let file = file(vec![fun("main", vec![if_stmt(int_lit(1), vec![], None)])]);
    let errors = lower_user(file).expect_err("Int condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "if condition must be Boolean, found Int");
}

#[test]
fn while_condition_must_be_boolean() {
    let file = file(vec![fun("main", vec![while_stmt(str_lit("x"), vec![])])]);
    let errors = lower_user(file).expect_err("String condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "while condition must be Boolean, found String"
    );
}
