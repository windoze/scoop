use super::*;

// --- resolution: applicability (DESIGN 1.2, steps 1-2) ---

#[test]
fn resolves_by_arity_and_type() {
    let show_overloads = vec![
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            str_lit("string"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int")), ("extra", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("two"),
        ),
    ];
    let file = file(
        show_overloads
            .into_iter()
            .chain([fun(
                "main",
                vec![
                    stmt(call("println", vec![call("show", vec![int_lit(1)])])),
                    stmt(call("println", vec![call("show", vec![str_lit("a")])])),
                    stmt(call(
                        "println",
                        vec![call("show", vec![int_lit(1), int_lit(2)])],
                    )),
                ],
            )])
            .collect(),
    );
    let module = lower_user(file).expect("overload resolution must succeed");
    let (first, _) = call_in_main(&module, 0, true);
    assert_eq!(first, top_level_fn(&module, "show", &["Int"]));
    let (second, _) = call_in_main(&module, 1, true);
    assert_eq!(second, top_level_fn(&module, "show", &["String"]));
    let (third, _) = call_in_main(&module, 2, true);
    assert_eq!(third, top_level_fn(&module, "show", &["Int", "Int"]));
}

#[test]
fn no_applicable_overload_lists_argument_types() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("String"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![bool_lit(true)]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("no applicable overload must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no overload of `f` matches argument types (Boolean)"
    );
}

#[test]
fn mixed_arity_no_match_lists_argument_types() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![str_lit("s")]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("no applicable overload must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no overload of `f` matches argument types (String)"
    );
}

#[test]
fn uniform_arity_mismatch_keeps_the_arity_message() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("String"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("arity mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`f` takes exactly 1 argument, but 0 were supplied"
    );
}
