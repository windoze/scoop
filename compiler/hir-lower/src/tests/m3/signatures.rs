use super::super::*;

// --- positive: signatures and returns ---

#[test]
fn expression_body_and_parameters() {
    let file = file(vec![
        fun_expr(
            "double",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            binary(BinOp::Mul, var("x"), int_lit(2)),
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("double", vec![int_lit(21)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("expression body must lower");
    let expected = include_str!("snapshots/expression_body_and_parameters.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn block_body_with_return_and_parameters() {
    let file = file(vec![
        fun_sig(
            "add",
            vec![],
            vec![("a", ty_named("Int")), ("b", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(binary(BinOp::Add, var("a"), var("b"))))],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("add", vec![int_lit(1), int_lit(2)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("block body must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("fun add(a: Int, b: Int): Int"), "{dump}");
    assert!(dump.contains("Call add : Int"), "{dump}");
}

/// `return <unit value>` in a `Unit` function evaluates the value and
/// lowers to a bare `return` (hir: `Return::value` is absent in `Unit`
/// functions).
#[test]
fn return_with_unit_value_is_a_bare_return() {
    let file = file(vec![
        fun("main", vec![stmt(call("f", vec![]))]),
        fun("f", vec![ret(Some(call("println", vec![str_lit("x")])))]),
    ]);
    let module = lower_user(file).expect("Unit return with value must lower");
    let expected = include_str!("snapshots/return_with_unit_value_is_a_bare_return.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}
