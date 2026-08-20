use super::*;

#[test]
fn throw_non_throwable_is_an_error() {
    let value_span = Span::new(10, 12);
    let value = Expr::IntLiteral(ast::IntegerLiteralSyntax {
        span: value_span,
        ..integer_syntax(42)
    });
    let file = file(vec![fun("main", vec![throw_stmt(value)])]);
    let errors = lower_user_with_exceptions(file).expect_err("throwing an Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot throw value of type Int: not a subtype of Throwable"
    );
    assert_eq!(errors[0].span, Some(value_span));
    // The user file follows the single complete core file.
    assert_eq!(errors[0].file, 1);
}

/// A non-Unit function may end with `throw` instead of `return`: the
/// throw diverges, so the function never leaves without a value (the
/// M3 return rule, relaxed in M8).
#[test]
fn trailing_throw_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "fail",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![throw_stmt(call("MyError", vec![int_lit(42)]))],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("a trailing `throw` must satisfy the return rule");
}

/// A `throw` makes the rest of its sequential block unreachable, so
/// following statements do not make the function fall through.
#[test]
fn non_trailing_throw_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                throw_stmt(call("MyError", vec![int_lit(1)])),
                val("y", int_lit(1)),
            ],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("statements after `throw` are unreachable");
}

/// A `try` satisfies the return rule when its try body and every catch
/// body do, while a normally completing `finally` preserves those
/// path results.
#[test]
fn trailing_try_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "outer",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![try_stmt(
                vec![ret(Some(int_lit(0)))],
                vec![catch_clause(
                    "e",
                    ty_named("MyError"),
                    vec![throw_stmt(var("e"))],
                )],
                Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
            )],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("a non-falling-through `try` must qualify");
}

/// A trailing `try` whose try body (or a catch body) falls through
/// still trips the M3 return rule.
#[test]
fn falling_through_try_does_not_satisfy_the_return_rule() {
    for (try_body, catch_body) in [
        // The try body falls through.
        (
            vec![stmt(call("println", vec![str_lit("x")]))],
            vec![ret(Some(int_lit(0)))],
        ),
        // A catch body falls through.
        (
            vec![ret(Some(int_lit(0)))],
            vec![stmt(call("println", vec![str_lit("x")]))],
        ),
    ] {
        let file = file(vec![
            custom_error(),
            fun_sig(
                "g",
                vec![],
                vec![],
                Some(ty_named("Int")),
                vec![try_stmt(
                    try_body,
                    vec![catch_clause("e", ty_named("MyError"), catch_body)],
                    Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
                )],
            ),
            fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
        ]);
        let errors =
            lower_user_with_exceptions(file).expect_err("a falling-through `try` must not qualify");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].message,
            "non-Unit function `g` may complete without returning a value"
        );
    }
}

/// A finally that cannot complete normally overrides every pending
/// normal, return, or exceptional path.
#[test]
fn exiting_finally_satisfies_the_return_rule() {
    for finally_body in [
        vec![ret(Some(int_lit(7)))],
        vec![throw_stmt(call("MyError", vec![int_lit(8)]))],
        vec![if_stmt(
            bool_lit(true),
            vec![ret(Some(int_lit(9)))],
            Some(vec![ret(Some(int_lit(10)))]),
        )],
    ] {
        let file = file(vec![
            custom_error(),
            fun_sig(
                "f",
                vec![],
                vec![],
                Some(ty_named("Int")),
                vec![try_stmt(
                    vec![stmt(call("println", vec![str_lit("body")]))],
                    vec![],
                    Some(finally_body),
                )],
            ),
            fun("main", vec![]),
        ]);
        lower_user_with_exceptions(file).expect("an exiting finally overrides every path");
    }
}

#[test]
fn partially_exiting_finally_does_not_hide_fallthrough() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![try_stmt(
                vec![stmt(call("println", vec![str_lit("body")]))],
                vec![],
                Some(vec![if_stmt(
                    bool_lit(true),
                    vec![ret(Some(int_lit(1)))],
                    None,
                )]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_exceptions(file)
        .expect_err("a partially exiting finally still has a fallthrough path");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-Unit function `f` may complete without returning a value"
    );
}

// --- negative: catch ---
