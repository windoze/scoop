use super::super::*;

// --- negative: signatures and returns ---

#[test]
fn return_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![ret(Some(str_lit("s")))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("return mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` value of `f` must be of type Int, found String"
    );
}

#[test]
fn non_unit_function_must_not_fall_through() {
    for body in [
        vec![],
        vec![stmt(call("println", vec![str_lit("x")]))],
        vec![if_stmt(bool_lit(true), vec![ret(Some(int_lit(1)))], None)],
    ] {
        let file = file(vec![
            fun_sig("f", vec![], vec![], Some(ty_named("Int")), body),
            fun("main", vec![]),
        ]);
        let errors = lower_user(file).expect_err("missing return must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "non-Unit function `f` may complete without returning a value"
        );
    }
}

#[test]
fn exhaustive_if_and_early_exit_satisfy_non_unit_return_rule() {
    for body in [
        vec![if_stmt(
            bool_lit(true),
            vec![ret(Some(int_lit(1)))],
            Some(vec![ret(Some(int_lit(2)))]),
        )],
        vec![
            ret(Some(int_lit(1))),
            stmt(call("println", vec![str_lit("unreachable")])),
        ],
    ] {
        let file = file(vec![
            fun_sig("f", vec![], vec![], Some(ty_named("Int")), body),
            fun("main", vec![]),
        ]);
        lower_user(file).expect("all reachable paths return a value");
    }
}

#[test]
fn bare_return_in_non_unit_function_is_an_error() {
    let file = file(vec![
        fun_sig("f", vec![], vec![], Some(ty_named("Int")), vec![ret(None)]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("bare return must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` without a value in function `f` returning Int"
    );
}

#[test]
fn return_value_must_match_unit() {
    let file = file(vec![
        fun("main", vec![]),
        fun("f", vec![ret(Some(int_lit(1)))]),
    ]);
    let errors = lower_user(file).expect_err("non-Unit value in Unit function must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` value of `f` must be of type Unit, found Int"
    );
}

#[test]
fn expression_body_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_expr("f", vec![], vec![], Some(ty_named("Int")), str_lit("s")),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("body mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "body of `f` must be of type Int, found String"
    );
}

#[test]
fn duplicate_type_parameter_is_an_error() {
    let file = file(vec![
        fun_sig("f", vec!["T", "T"], vec![], None, vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate type parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate type parameter `T`");
}

#[test]
fn duplicate_parameter_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Int")), ("x", ty_named("Int"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate parameter `x`");
}

#[test]
fn generic_main_is_an_error() {
    let file = file(vec![fun_sig("main", vec!["T"], vec![], None, vec![])]);
    let errors = lower_user(file).expect_err("generic main must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`main` must not be generic");
}

// --- negative: generics ---

#[test]
fn unbound_type_argument_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec!["T"],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("x")))],
        ),
        fun(
            "main",
            vec![stmt(call("println", vec![call("f", vec![int_lit(1)])]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("unbound type argument must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `f` in current-unit top-level candidate layer:\n  - fun f<T>(x: Int): Int — cannot infer a unique type argument for `T`"
    );
}

#[test]
fn conflicting_type_arguments_are_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec!["T"],
            vec![("a", ty_named("T")), ("b", ty_named("T"))],
            Some(ty_named("T")),
            var("a"),
        ),
        fun(
            "main",
            vec![val("x", call("f", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("conflicting bindings must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `f` in current-unit top-level candidate layer:\n  - fun f<T>(a: T, b: T): T — conflicting types for `T`: Int and String"
    );
}

#[test]
fn argument_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun("main", vec![stmt(call("f", vec![str_lit("s")]))]),
    ]);
    let errors = lower_user(file).expect_err("argument mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `f` in current-unit top-level candidate layer:\n  - fun f(x: Int): Int — argument for `x` has type String, which is not a subtype of Int"
    );
}

/// Type parameters support no concrete operations beyond `==` / `!=`:
/// `T` is unconstrained at the definition site.
#[test]
fn arithmetic_on_a_type_parameter_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            binary(BinOp::Add, var("x"), int_lit(1)),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("arithmetic on `T` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `+` requires Int operands, found T and Int"
    );
}

// --- negative: Option ---

#[test]
fn null_assert_on_non_option_is_an_error() {
    let file = file(vec![fun("main", vec![val("x", null_assert(int_lit(1)))])]);
    let errors = lower_user(file).expect_err("`!!` on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`!!` requires an Option operand, found Int"
    );
}

#[test]
fn safe_field_access_on_non_option_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("x", safe_field(var("p"), "x")),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("`?.` on Point must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` requires an Option receiver, found Point"
    );
}

#[test]
fn elvis_on_non_option_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", elvis(int_lit(1), int_lit(0)))],
    )]);
    let errors = lower_user(file).expect_err("`?:` on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?:` requires an Option left-hand side, found Int"
    );
}

#[test]
fn elvis_right_hand_side_must_match() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            val("x", elvis(var("a"), str_lit("s"))),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis rhs mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "right-hand side of `?:` must be of type Int, found String"
    );
}

#[test]
fn none_without_expected_type_is_an_error() {
    for init in [none(), some(none())] {
        let file = file(vec![fun("main", vec![val("x", init)])]);
        let errors = lower_user(file).expect_err("untyped `None` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "cannot infer the type of `None`");
    }
}

#[test]
fn some_arity_is_an_error() {
    let file = file(vec![fun("main", vec![val("x", call("Some", vec![]))])]);
    let errors = lower_user(file).expect_err("`Some` arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Some` of `Option` takes exactly 1 argument, but 0 were supplied"
    );
}

/// A while condition is re-evaluated per iteration, but the `?.`/`?:`
/// desugaring statements would execute once before the loop — rejected
/// instead of silently changing evaluation semantics.
#[test]
fn desugaring_in_while_condition_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "a",
                Some(ty_nullable(ty_named("Boolean"))),
                some(bool_lit(true)),
            ),
            while_stmt(elvis(var("a"), bool_lit(false)), vec![]),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis in while condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` and `?:` are not allowed in a while condition"
    );
}
