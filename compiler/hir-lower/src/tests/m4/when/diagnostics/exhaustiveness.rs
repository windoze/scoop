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
fn desugaring_in_when_guard_is_retained_as_guard_setup() {
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
    let module = lower_user(file).expect("elvis setup must stay inside the guard path");
    let hir::FunctionKind::User(main) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    let when = main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("when statement");
    let guard = when.arms[0].guard.as_ref().expect("guard");
    assert!(
        guard
            .setup
            .iter()
            .any(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
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
        "non-exhaustive when: missing pattern Green"
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
        "non-exhaustive when: missing pattern Red; guarded arms do not contribute to exhaustiveness"
    );
}

#[test]
fn literal_payload_does_not_cover_the_whole_variant() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            when_stmt(
                var("o"),
                vec![
                    arm(
                        pat_pos(&["Some"], vec![pat_lit(int_lit(0))], None),
                        None,
                        vec![],
                    ),
                    arm(pat_bind("None"), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("Some(0) does not cover Some(1)");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Some(-9223372036854775808)"
    );
}

#[test]
fn value_when_requires_irrefutable_variant_payload_coverage() {
    let when = ast::Expr::When(Box::new(ast::When {
        subject: var("o"),
        arms: vec![
            arm(
                pat_pos(&["Some"], vec![pat_lit(int_lit(0))], None),
                None,
                vec![stmt(int_lit(1))],
            ),
            arm(pat_bind("None"), None, vec![stmt(int_lit(2))]),
        ],
        else_body: None,
        span: sp(),
    }));
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            val("result", when),
        ],
    )]);
    let errors = lower_user(file).expect_err("a value when must use the same sound proof");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Some(-9223372036854775808)"
    );
}

#[test]
fn literal_payload_does_not_cover_a_single_variant_enum() {
    let file = file(vec![
        enum_decl(
            "Only",
            vec![],
            vec![variant_positional("Value", vec![ty_named("Boolean")])],
        ),
        fun(
            "main",
            vec![
                val("only", call("Only.Value", vec![bool_lit(false)])),
                when_stmt(
                    var("only"),
                    vec![arm(
                        pat_pos(&["Value"], vec![pat_lit(bool_lit(true))], None),
                        None,
                        vec![],
                    )],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("Value(true) does not cover Value(false)");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Value(false)"
    );
}

#[test]
fn one_tuple_witness_keeps_the_required_trailing_comma() {
    let file = file(vec![
        enum_decl(
            "OnlyTuple",
            vec![],
            vec![variant_positional(
                "Value",
                vec![ty_tuple(vec![ty_named("Boolean")])],
            )],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("value", ty_named("OnlyTuple"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(
                    pat_pos(
                        &["Value"],
                        vec![pat_tuple(vec![pat_lit(bool_lit(true))], None)],
                        None,
                    ),
                    None,
                    vec![],
                )],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("Value((true,)) misses Value((false,))");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Value((false,))"
    );
}

#[test]
fn nested_literal_and_guarded_payloads_do_not_contribute_coverage() {
    let pair_ty = ty_tuple(vec![ty_named("Int"), ty_named("Boolean")]);
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "o",
                Some(ty_nullable(pair_ty)),
                some(tuple_lit(vec![int_lit(1), bool_lit(false)])),
            ),
            when_stmt(
                var("o"),
                vec![
                    arm(
                        pat_pos(
                            &["Some"],
                            vec![pat_tuple(vec![pat_bind("n"), pat_wild()], None)],
                            None,
                        ),
                        Some(bool_lit(true)),
                        vec![],
                    ),
                    arm(
                        pat_pos(
                            &["Some"],
                            vec![pat_tuple(vec![pat_lit(int_lit(0)), pat_wild()], None)],
                            None,
                        ),
                        None,
                        vec![],
                    ),
                    arm(pat_bind("None"), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("guarded and nested literal arms stay refutable");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Some((-9223372036854775808, false)); guarded arms do not contribute to exhaustiveness"
    );
}

#[test]
fn recursively_irrefutable_payloads_produce_an_enum_proof() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            when_stmt(
                var("o"),
                vec![
                    arm(
                        pat_pos(&["Some"], vec![pat_bind("value")], None),
                        None,
                        vec![],
                    ),
                    arm(pat_bind("None"), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let output = lower_user_output(file).expect("Some(binding) and None are exhaustive");
    let module = &output.export;
    let hir::FunctionKind::User(main) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    let when = main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("when statement");
    assert!(matches!(
        &when.fallback,
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix { .. })
    ));
    let concrete = &output.local;
    let hir::concrete::FunctionKind::User(main) = &concrete.functions[concrete.entry].kind else {
        panic!("concrete main body")
    };
    let when = main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("concrete when statement");
    assert!(matches!(
        &when.fallback,
        hir::concrete::WhenFallback::Impossible(
            hir::concrete::ExhaustivenessProof::EnumPatternMatrix { .. }
        )
    ));
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
        "non-exhaustive when: missing pattern (-9223372036854775808, -9223372036854775808)"
    );
}

#[test]
fn open_string_domain_reports_the_first_fresh_literal() {
    let file = file(vec![
        enum_decl(
            "Text",
            vec![],
            vec![variant_positional("Value", vec![ty_named("String")])],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("text", ty_named("Text"))],
            None,
            vec![when_stmt(
                var("text"),
                vec![arm(
                    pat_pos(&["Value"], vec![pat_lit(str_lit(""))], None),
                    None,
                    vec![],
                )],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a finite set of String literals is not exhaustive");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern Value(\"a\")"
    );
}

#[test]
fn recursive_value_layouts_are_rejected_before_when_proof() {
    let file = file(vec![
        enum_decl(
            "Direct",
            vec![],
            vec![
                variant_positional("Again", vec![ty_named("Direct")]),
                variant_unit("DirectEnd"),
            ],
        ),
        enum_decl(
            "Left",
            vec![],
            vec![
                variant_positional("GoRight", vec![ty_named("Right")]),
                variant_unit("LeftEnd"),
            ],
        ),
        enum_decl(
            "Right",
            vec![],
            vec![
                variant_positional("GoLeft", vec![ty_named("Left")]),
                variant_unit("RightEnd"),
            ],
        ),
        enum_decl(
            "Grow",
            vec!["T"],
            vec![
                variant_positional(
                    "More",
                    vec![ty_generic(
                        "Grow",
                        vec![ty_tuple(vec![ty_named("T"), ty_named("T")])],
                    )],
                ),
                variant_unit("GrowEnd"),
            ],
        ),
        generic_struct_decl("Carrier", vec!["T"], vec![("value", ty_named("T"))]),
        struct_decl(
            "Recursive",
            vec![("value", ty_generic("Carrier", vec![ty_named("Recursive")]))],
        ),
        fun_sig(
            "checkDirect",
            vec![],
            vec![("value", ty_named("Direct"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(pat_bind("DirectEnd"), None, vec![])],
                None,
            )],
        ),
        fun_sig(
            "checkLeft",
            vec![],
            vec![("value", ty_named("Left"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(pat_bind("LeftEnd"), None, vec![])],
                None,
            )],
        ),
        fun_sig(
            "checkGrow",
            vec![],
            vec![("value", ty_generic("Grow", vec![ty_named("Int")]))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(pat_bind("GrowEnd"), None, vec![])],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("by-value declaration cycles must be rejected");
    assert_eq!(
        errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "recursive value layout does not cross a reference boundary: Recursive -> Recursive",
            "recursive value layout does not cross a reference boundary: Direct -> Direct",
            "recursive value layout does not cross a reference boundary: Left -> Right -> Left",
            "recursive value layout does not cross a reference boundary: Grow -> Grow",
        ]
    );
}
