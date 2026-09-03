use super::*;

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
