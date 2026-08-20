use super::super::*;

// --- positive: operators and control flow ---

#[test]
fn var_rebinding_and_control_flow() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("n", int_lit(0)),
            while_stmt(
                binary(BinOp::Lt, var("n"), int_lit(3)),
                vec![assign("n", binary(BinOp::Add, var("n"), int_lit(1)))],
            ),
            if_stmt(
                binary(
                    BinOp::And,
                    binary(BinOp::Eq, var("n"), int_lit(3)),
                    bool_lit(true),
                ),
                vec![stmt(call("println", vec![str_lit("ok")]))],
                Some(vec![stmt(call("println", vec![str_lit("ng")]))]),
            ),
        ],
    )]);
    let module = lower_user(file).expect("control flow program must lower");
    let expected = include_str!("snapshots/var_rebinding_and_control_flow.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn unary_operators_and_all_printables() {
    let file = file(vec![fun(
        "main",
        vec![
            val("n", unary(UnOp::Neg, int_lit(5))),
            val("b", unary(UnOp::Not, bool_lit(false))),
            val(
                "m",
                binary(
                    BinOp::Sub,
                    binary(BinOp::Mul, var("n"), int_lit(2)),
                    int_lit(1),
                ),
            ),
            val("d", binary(BinOp::Div, var("m"), int_lit(3))),
            val("le", binary(BinOp::Le, var("d"), int_lit(0))),
            val("ge", binary(BinOp::Ge, var("d"), int_lit(0))),
            val("gt", binary(BinOp::Gt, var("d"), int_lit(0))),
            val("ne", binary(BinOp::Ne, var("le"), var("ge"))),
            val("or", binary(BinOp::Or, var("gt"), var("ne"))),
            stmt(call("println", vec![int_lit(42)])),
            stmt(call("println", vec![bool_lit(true)])),
            stmt(call("print", vec![var("or")])),
        ],
    )]);
    lower_user(file).expect("operator program must lower");
}

// --- positive: scopes ---

#[test]
fn inner_scopes_shadow_and_do_not_leak() {
    let file = file(vec![fun(
        "main",
        vec![
            val("x", int_lit(1)),
            if_stmt(
                bool_lit(true),
                vec![
                    val("x", str_lit("inner")),
                    stmt(call("println", vec![var("x")])),
                ],
                None,
            ),
            stmt(call("println", vec![var("x")])),
            block_stmt(vec![
                val("y", int_lit(2)),
                stmt(call("println", vec![var("y")])),
            ]),
        ],
    )]);
    let module = lower_user(file).expect("shadowing program must lower");
    let expected = include_str!("snapshots/inner_scopes_shadow_and_do_not_leak.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}
