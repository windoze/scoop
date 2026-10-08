use super::*;

// --- positive: destructuring declarations ---

#[test]
fn destructuring_declarations() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                // Tuple, with `..` skipping the middle elements.
                val(
                    "t",
                    tuple_lit(vec![int_lit(1), int_lit(2), int_lit(3), int_lit(4)]),
                ),
                val_pat(
                    false,
                    ast::Pattern::Tuple {
                        elements: vec![
                            pat_bind_at("a", Span::new(10, 11)),
                            pat_bind_at("b", Span::new(30, 31)),
                        ],
                        rest: Some(Span::new(20, 22)),
                        span: sp(),
                    },
                    None,
                    var("t"),
                ),
                // Struct field pattern with rename and `..`.
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val_pat(
                    false,
                    pat_named(&["Point"], vec![("x", Some("px"))], Some(sp())),
                    None,
                    var("p"),
                ),
                // Struct positional pattern, `var` bindings are mutable.
                var_pat2(pat_tuple(vec![pat_bind("qx"), pat_wild()], None), var("p")),
                // Nested tuple-in-tuple.
                val(
                    "nested",
                    tuple_lit(vec![tuple_lit(vec![int_lit(1), int_lit(2)]), str_lit("s")]),
                ),
                val_pat(
                    false,
                    pat_tuple(
                        vec![
                            pat_tuple(vec![pat_bind("m"), pat_bind("n")], None),
                            pat_wild(),
                        ],
                        None,
                    ),
                    None,
                    var("nested"),
                ),
                stmt(call("println", vec![var("a")])),
                stmt(call("println", vec![var("b")])),
                stmt(call("println", vec![var("px")])),
                stmt(call("println", vec![var("m")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("destructuring must lower");
    let expected = include_str!("snapshots/destructuring_declarations.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

/// `var` destructuring: bindings are mutable.
fn var_pat2(target: ast::Pattern, init: Expr) -> Statement {
    val_pat(true, target, None, init)
}

// --- negative: destructuring declarations ---

#[test]
fn refutable_patterns_in_val_are_an_error() {
    for (target, init) in [
        (pat_pos(&["Some"], vec![pat_bind("x")], None), var("o")),
        (pat_lit(int_lit(0)), var("o")),
        // Nested refutable pattern inside an irrefutable one.
        (
            pat_tuple(
                vec![pat_pos(&["Some"], vec![pat_bind("x")], None), pat_wild()],
                None,
            ),
            tuple_lit(vec![var("o"), int_lit(2)]),
        ),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
                val_pat(false, target, None, init),
            ],
        )]);
        let errors = lower_user(file).expect_err("refutable pattern in val must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }
}

#[test]
fn destructuring_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_pat(
            false,
            pat_tuple(vec![pat_bind("x"), pat_bind("y")], None),
            None,
            tuple_lit(vec![int_lit(1)]),
        )],
    )]);
    let errors = lower_user(file).expect_err("pattern/type mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 2 element(s), but tuple of type (Int) has 1"
    );
}

#[test]
fn literal_pattern_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), str_lit("s")])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(str_lit("x")), pat_bind("s")], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("literal mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "literal pattern of type String cannot match Int"
    );
}

#[test]
fn non_literal_literal_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1)])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(call("f", vec![]))], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-literal pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "expected a literal pattern");
}

#[test]
fn duplicate_binding_in_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                vec![arm(
                    pat_tuple(vec![pat_bind("x"), pat_bind("x")], None),
                    None,
                    vec![],
                )],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("duplicate binding must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`x` is already declared in this scope");
}
