use super::super::*;

// --- negative: when ---

#[test]
fn when_subject_must_be_patternable() {
    let file = file(vec![fun(
        "main",
        vec![when_stmt(
            int_lit(1),
            vec![arm(pat_wild(), None, vec![])],
            None,
        )],
    )]);
    let errors = lower_user(file).expect_err("Int subject must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`when` subject must be an enum, tuple or struct, found Int"
    );
}

#[test]
fn unknown_variant_pattern_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_pos(&["Purple"], vec![], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown variant pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

#[test]
fn pattern_path_subject_mismatch_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("t", tuple_lit(vec![int_lit(1)])),
                when_stmt(
                    var("t"),
                    vec![arm(pat_pos(&["Color", "Red"], vec![], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("mismatched path must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern `Color.Red` does not match a subject of type (Int)"
    );
}

#[test]
fn variant_pattern_arity_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(
                            pat_pos(&["Circle"], vec![pat_bind("r"), pat_bind("q")], None),
                            None,
                            vec![],
                        ),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("pattern arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 2 element(s), but variant `Circle` of `Shape` has 1"
    );
}

#[test]
fn tuple_pattern_arity_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                vec![arm(pat_tuple(vec![pat_bind("x")], None), None, vec![])],
                Some(vec![]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("tuple arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 1 element(s), but tuple of type (Int, Int) has 2"
    );
}

#[test]
fn tuple_pattern_against_enum_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_tuple(vec![pat_bind("x")], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("tuple pattern on enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple pattern does not match a subject of type Color"
    );
}

#[test]
fn named_pattern_missing_rest_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("missing `..` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern does not list all fields of struct `Point`; add `..` to ignore the rest"
    );
}

#[test]
fn named_pattern_unknown_field_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None), ("z", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `z`");
}

#[test]
fn named_pattern_underscore_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("_", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("`_` in field pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`_` is not allowed in a field pattern");
}

#[test]
fn duplicate_field_in_pattern_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None), ("x", Some("y"))], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("duplicate field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate field `x` in pattern");
}

#[test]
fn positional_pattern_on_named_variant_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Named", vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(
                            pat_pos(&["Named"], vec![pat_bind("w"), pat_bind("h")], None),
                            None,
                            vec![],
                        ),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("positional on named must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Named` of `Shape` has named fields; use a named field pattern"
    );
}

#[test]
fn named_pattern_on_positional_variant_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(pat_named(&["Circle"], vec![], Some(sp())), None, vec![]),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("named on positional must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Circle` of `Shape` has no named fields; use a positional pattern"
    );
}

#[test]
fn field_pattern_without_name_on_enum_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![arm(pat_named(&[], vec![], Some(sp())), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("nameless field pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "a field pattern without a type name can only match a struct, found Shape"
    );
}

#[test]
fn when_guard_must_be_boolean() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_bind("Red"), Some(int_lit(1)), vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("Int guard must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "when guard must be Boolean, found Int");
}

#[test]
fn desugaring_in_when_guard_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "o",
                Some(ty_nullable(ty_named("Boolean"))),
                some(bool_lit(true)),
            ),
            when_stmt(
                var("o"),
                vec![arm(
                    pat_pos(&["Some"], vec![pat_bind("b")], None),
                    Some(elvis(var("o"), bool_lit(false))),
                    vec![],
                )],
                Some(vec![]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis in guard must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` and `?:` are not allowed in a when guard"
    );
}

#[test]
fn non_exhaustive_when_reports_missing_variants() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(var("c"), vec![arm(pat_bind("Red"), None, vec![])], None),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("non-exhaustive when must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing variant(s) `Green`, `Blue`"
    );
}

#[test]
fn guarded_arm_does_not_count_for_exhaustiveness() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![
                        arm(pat_bind("Red"), Some(bool_lit(true)), vec![]),
                        arm(pat_bind("Green"), None, vec![]),
                        arm(pat_bind("Blue"), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("guarded arm must not count");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing variant(s) `Red`"
    );
}

#[test]
fn non_exhaustive_tuple_when_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                // A literal makes the arm refutable.
                vec![arm(
                    pat_tuple(vec![pat_lit(int_lit(0)), pat_bind("x")], None),
                    None,
                    vec![],
                )],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-exhaustive tuple when must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: add a catch-all pattern or an `else` branch"
    );
}
