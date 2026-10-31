use super::*;

fn signal_decl() -> Decl {
    enum_decl(
        "Signal",
        vec![],
        vec![variant_unit("Ready"), variant_unit("Failed")],
    )
}

#[test]
fn unknown_bare_enum_pattern_binds_without_a_warning() {
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
                        arm(pat_bind("Raedy"), None, vec![val("matched", var("Raedy"))]),
                    ],
                    None,
                ),
            ],
        ),
    ]))
    .expect("an unknown bare enum name introduces a usable binding");

    assert!(output.warnings.is_empty());
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
    .expect("variant, wildcard and binding patterns remain valid");

    assert!(output.warnings.is_empty());
}

#[test]
fn enum_binding_does_not_add_a_warning_to_a_later_error() {
    let diagnostics = lower_user_output(file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val("signal", field(var("Signal"), "Ready")),
                when_stmt(
                    var("signal"),
                    vec![arm(pat_bind("Raedy"), None, vec![])],
                    None,
                ),
                val("broken", var("missing")),
            ],
        ),
    ]))
    .expect_err("a valid enum binding must not hide a later semantic error");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, ast::DiagnosticSeverity::Error);
    assert!(
        diagnostics[0]
            .message
            .contains("unknown variable `missing`")
    );
}

#[test]
fn recursive_enum_payloads_bind_without_warnings() {
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
                        pat_pos(&["Wrap"], vec![pat_bind("first"), pat_bind("second")], None),
                        None,
                        vec![val("pair", tuple_lit(vec![var("first"), var("second")]))],
                    )],
                    None,
                ),
            ],
        ),
    ]))
    .expect("recursive catch-all bindings remain valid patterns");

    assert!(output.warnings.is_empty());
}

#[test]
fn contextual_value_arm_preserves_enum_bindings_without_warnings() {
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
                        subject: ast::WhenSubject::Expression(var("input")),
                        arms: vec![
                            arm(
                                pat_bind("First"),
                                None,
                                vec![stmt(field(var("Outcome"), "Done"))],
                            ),
                            arm(
                                pat_bind("Firts"),
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

    assert!(output.warnings.is_empty());
}

#[test]
fn invalid_constructor_argument_reports_only_its_type_error() {
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
                            subject: ast::WhenSubject::Expression(var("signal")),
                            arms: vec![arm(
                                pat_bind("Raedy"),
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
