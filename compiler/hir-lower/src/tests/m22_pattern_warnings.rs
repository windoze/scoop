use super::*;

fn signal_decl() -> Decl {
    enum_decl(
        "Signal",
        vec![],
        vec![variant_unit("Ready"), variant_unit("Failed")],
    )
}

#[test]
fn unknown_bare_enum_pattern_is_a_non_fatal_warning() {
    let typo_span = Span::new(41, 47);
    let output = lower_user_output(file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                when_stmt(
                    var("signal"),
                    vec![
                        arm(pat_bind("Ready"), None, vec![]),
                        arm(pat_bind_at("Raedy", typo_span), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]))
    .expect("a catch-all warning must not prevent HIR output");

    assert_eq!(output.warnings.len(), 1);
    let warning = &output.warnings[0];
    assert_eq!(warning.severity, ast::DiagnosticSeverity::Warning);
    assert_eq!(warning.file, 1);
    assert_eq!(warning.span, Some(typo_span));
    assert_eq!(
        warning.message,
        "`Raedy` is a catch-all binding because `Signal` has no variant named `Raedy`; qualify an intended variant as `E.V`, or use `_` or an intentional binding name for a catch-all"
    );
}

#[test]
fn resolved_variant_wildcard_non_enum_and_non_when_bindings_do_not_warn() {
    let output = lower_user_output(file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                when_stmt(
                    var("signal"),
                    vec![
                        arm(pat_bind("Ready"), None, vec![]),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
                val("pair", tuple_lit(vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("pair"),
                    vec![arm(pat_bind("wholePair"), None, vec![])],
                    None,
                ),
                val_pat(
                    false,
                    pat_bind("ordinaryBinding"),
                    None,
                    field(var("Signal"), "Failed"),
                ),
            ],
        ),
    ]))
    .expect("none of the patterns is a suspicious enum catch-all binding");

    assert!(output.warnings.is_empty());
}

#[test]
fn committed_warning_survives_a_later_error_without_changing_failure() {
    let typo_span = Span::new(70, 76);
    let diagnostics = lower_user_output(file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                when_stmt(
                    var("signal"),
                    vec![arm(pat_bind_at("Raedy", typo_span), None, vec![])],
                    None,
                ),
                val("broken", var("missing")),
            ],
        ),
    ]))
    .expect_err("a warning must not turn a later semantic error into success");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].severity, ast::DiagnosticSeverity::Error);
    assert!(
        diagnostics[0]
            .message
            .contains("unknown variable `missing`")
    );
    assert_eq!(diagnostics[1].severity, ast::DiagnosticSeverity::Warning);
    assert_eq!(diagnostics[1].span, Some(typo_span));
    assert!(diagnostics[1].message.contains("catch-all binding"));
}

#[test]
fn recursive_enum_columns_warn_once_for_each_binding_in_source_order() {
    let first_span = Span::new(20, 25);
    let second_span = Span::new(30, 36);
    let output = lower_user_output(file(vec![
        signal_decl(),
        enum_decl(
            "Pair",
            vec![],
            vec![variant_positional(
                "Wrap",
                vec![ty_named("Signal"), ty_named("Signal")],
            )],
        ),
        fun(
            "main",
            vec![
                val(
                    "pair",
                    call(
                        "Pair.Wrap",
                        vec![
                            field(var("Signal"), "Ready"),
                            field(var("Signal"), "Failed"),
                        ],
                    ),
                ),
                when_stmt(
                    var("pair"),
                    vec![arm(
                        pat_pos(
                            &["Wrap"],
                            vec![
                                pat_bind_at("first", first_span),
                                pat_bind_at("second", second_span),
                            ],
                            None,
                        ),
                        None,
                        vec![],
                    )],
                    None,
                ),
            ],
        ),
    ]))
    .expect("recursive catch-all bindings remain valid patterns");

    assert_eq!(output.warnings.len(), 2);
    assert_eq!(output.warnings[0].span, Some(first_span));
    assert_eq!(output.warnings[1].span, Some(second_span));
    assert!(
        output
            .warnings
            .iter()
            .all(|warning| warning.severity == ast::DiagnosticSeverity::Warning)
    );
}

#[test]
fn value_arm_classification_probe_does_not_duplicate_a_warning() {
    let typo_span = Span::new(50, 56);
    let output = lower_user_output(file(vec![
        enum_decl(
            "Input",
            vec![],
            vec![variant_unit("First"), variant_unit("Second")],
        ),
        enum_decl("Outcome", vec![], vec![variant_unit("Done")]),
        fun_sig(
            "consume",
            vec![],
            vec![("value", ty_named("Input"))],
            None,
            vec![],
        ),
        fun(
            "main",
            vec![
                val("input", field(var("Input"), "First")),
                val(
                    "result",
                    Expr::When(Box::new(ast::When {
                        subject: var("input"),
                        arms: vec![
                            arm(
                                pat_bind("First"),
                                None,
                                vec![stmt(field(var("Outcome"), "Done"))],
                            ),
                            arm(
                                pat_bind_at("Firts", typo_span),
                                None,
                                vec![stmt(call("consume", vec![var("Firts")])), stmt(var("Done"))],
                            ),
                        ],
                        else_body: None,
                        span: sp(),
                    })),
                ),
            ],
        ),
    ]))
    .expect("the qualified arm fixes the contextual result type");

    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].span, Some(typo_span));
}

#[test]
fn failed_single_nominal_constructor_probe_does_not_leak_a_warning() {
    let errors = lower_user_output(file(vec![
        signal_decl(),
        struct_decl("Box", vec![("value", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                val(
                    "boxed",
                    struct_init(
                        "Box",
                        vec![Expr::When(Box::new(ast::When {
                            subject: var("signal"),
                            arms: vec![arm(
                                pat_bind_at("Raedy", Span::new(60, 66)),
                                None,
                                vec![stmt(str_lit("not an Int"))],
                            )],
                            else_body: None,
                            span: sp(),
                        }))],
                    ),
                ),
            ],
        ),
    ]))
    .expect_err("the constructor argument has the wrong type");

    assert!(!errors.is_empty());
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.severity == ast::DiagnosticSeverity::Error)
    );
    assert!(
        errors
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("catch-all binding"))
    );
}

#[test]
fn qualified_unknown_variant_remains_an_error_without_a_warning_fallback() {
    let errors = lower_user_output(file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                when_stmt(
                    var("signal"),
                    vec![arm(
                        pat_pos(&["Signal", "Raedy"], vec![], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]))
    .expect_err("a qualified unknown variant must not become a binding");

    assert!(errors.iter().any(|diagnostic| {
        diagnostic.severity == ast::DiagnosticSeverity::Error
            && diagnostic.message == "enum `Signal` has no variant `Raedy`"
    }));
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.severity != ast::DiagnosticSeverity::Warning)
    );
}
