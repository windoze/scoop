use super::*;

// --- declaration rules (DESIGN 1.1) ---

#[test]
fn distinguishable_overloads_are_accepted() {
    let file = file(vec![
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
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("distinguishable overloads must lower");
    top_level_fn(&module, "show", &["Int"]);
    top_level_fn(&module, "show", &["String"]);
    top_level_fn(&module, "show", &["Int", "Int"]);
}

#[test]
fn same_signature_duplicate_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("y", ty_named("Int"))],
            Some(ty_named("Int")),
            var("y"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature overloads must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn differing_only_in_return_type_is_a_duplicate() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("s"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("return-type-only difference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn same_signature_method_duplicate_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr(
                    "m",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("x"),
                ),
                method_expr(
                    "m",
                    vec![("y", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("y"),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature methods must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn override_with_unmatched_signature_is_an_error() {
    // An overload of `m` exists in the base class, but with a
    // different signature — it is not overridden.
    let file = file(vec![
        class_decl(
            Open,
            "B",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "m",
                vec![("x", ty_named("Int"))],
                Some(ty_named("String")),
                str_lit("i"),
            )],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "m",
                vec![("x", ty_named("String"))],
                Some(ty_named("String")),
                var("x"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unmatched override must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}
