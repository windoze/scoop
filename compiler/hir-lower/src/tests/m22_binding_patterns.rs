use super::*;

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    module
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == name).then(|| match &function.kind {
                hir::FunctionKind::User(body) => body,
                _ => panic!("`{name}` must have a user body"),
            })
        })
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn local_names(body: &hir::Body) -> Vec<&str> {
    body.locals
        .iter()
        .map(|(_, local)| local.name.as_str())
        .collect()
}

fn signal_decl() -> Decl {
    enum_decl(
        "Signal",
        vec![],
        vec![
            variant_unit("Ready"),
            variant_positional("Payload", vec![ty_named("Int")]),
        ],
    )
}

#[test]
fn bare_enum_variant_names_bind_in_plain_val_targets() {
    let source = file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val_ty("optional", Some(ty_nullable(ty_named("Int"))), none()),
                val_pat(false, pat_bind("None"), None, var("optional")),
                val("signal", field(var("Signal"), "Ready")),
                val_pat(false, pat_bind("Ready"), None, var("signal")),
            ],
        ),
    ]);

    let module = lower_user(source).expect("bare variant names must introduce val bindings");
    let names = local_names(function_body(&module, "main"));
    assert!(names.contains(&"None"));
    assert!(names.contains(&"Ready"));
}

#[test]
fn bare_enum_variant_names_bind_at_nested_struct_and_tuple_depths() {
    let source = file(vec![
        signal_decl(),
        struct_decl(
            "Bundle",
            vec![
                ("optional", ty_nullable(ty_named("Int"))),
                ("signal", ty_named("Signal")),
                (
                    "nested",
                    ty_tuple(vec![ty_named("Signal"), ty_nullable(ty_named("Int"))]),
                ),
            ],
        ),
        fun(
            "main",
            vec![
                val(
                    "bundle",
                    struct_init(
                        "Bundle",
                        vec![
                            some(int_lit(1)),
                            field(var("Signal"), "Ready"),
                            tuple_lit(vec![call("Signal.Payload", vec![int_lit(2)]), none()]),
                        ],
                    ),
                ),
                val_pat(
                    false,
                    pat_named_subpatterns(
                        &[],
                        vec![
                            ("optional", pat_bind("None")),
                            ("signal", pat_bind("Ready")),
                            (
                                "nested",
                                pat_tuple(vec![pat_bind("Payload"), pat_bind("NestedNone")], None),
                            ),
                        ],
                        None,
                    ),
                    None,
                    var("bundle"),
                ),
            ],
        ),
    ]);

    let module = lower_user(source).expect("nested bare variant names must introduce bindings");
    let names = local_names(function_body(&module, "main"));
    for expected in ["None", "Ready", "Payload", "NestedNone"] {
        assert!(names.contains(&expected), "missing local `{expected}`");
    }
}

#[test]
fn val_binding_plan_emits_source_order_projections_and_immutable_temporaries() {
    let source = file(vec![
        struct_decl(
            "Record",
            vec![
                ("first", ty_named("Int")),
                ("second", ty_named("Int")),
                ("third", ty_named("Int")),
            ],
        ),
        fun(
            "main",
            vec![val_pat(
                true,
                pat_named_subpatterns(
                    &[],
                    vec![
                        ("third", pat_bind("thirdValue")),
                        ("first", pat_bind("firstValue")),
                        ("second", pat_wild()),
                    ],
                    None,
                ),
                None,
                struct_init("Record", vec![int_lit(1), int_lit(2), int_lit(3)]),
            )],
        ),
    ]);

    let module = lower_user(source).expect("the val binding plan must lower");
    let body = function_body(&module, "main");
    let plan_start = body
        .statements
        .iter()
        .position(|statement| {
            let hir::StatementKind::ValDecl { pattern, .. } = &statement.kind else {
                return false;
            };
            let hir::Pattern::Binding { local } = pattern else {
                return false;
            };
            body.locals[*local].name.starts_with("$binding.subject.")
        })
        .expect("the plan has an immutable subject temporary");
    let plan = &body.statements[plan_start..];
    assert!(plan.iter().all(|statement| matches!(
        &statement.kind,
        hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { .. },
            ..
        }
    )));

    let projection_indices = plan
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            let hir::ExprKind::FieldAccess {
                field: hir::FieldRef::StructField { field, .. },
                ..
            } = &init.kind
            else {
                return None;
            };
            Some(
                module
                    .field_identities
                    .struct_declaration(*field)
                    .unwrap()
                    .local_index(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        projection_indices,
        vec![2, 0],
        "named projections follow source order and an explicit `_` is ignored",
    );

    for (_, local) in body.locals.iter() {
        if local.name.starts_with("$binding.subject.")
            || local.name.starts_with("$binding.projection.")
        {
            assert!(!local.mutable, "hidden binding temporaries stay immutable");
        }
        if matches!(local.name.as_str(), "thirdValue" | "firstValue") {
            assert!(local.mutable, "only user-visible `var` leaves are mutable");
        }
    }
}

#[test]
fn bare_enum_variant_names_bind_in_composite_lambda_parameters() {
    let operation = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_tuple(vec![pat_bind("None"), pat_bind("Ready")], None),
            ty: None,
            span: sp(),
        }]),
        body: block(vec![stmt(tuple_lit(vec![var("None"), var("Ready")]))]),
        span: sp(),
    };
    let source = file(vec![
        signal_decl(),
        fun(
            "main",
            vec![val_ty(
                "operation",
                Some(ty_function(
                    false,
                    vec![ty_tuple(vec![
                        ty_nullable(ty_named("Int")),
                        ty_named("Signal"),
                    ])],
                    ty_tuple(vec![ty_nullable(ty_named("Int")), ty_named("Signal")]),
                )),
                operation,
            )],
        ),
    ]);

    let module = lower_user(source).expect("lambda patterns use binding-context name lookup");
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    let hir::FunctionKind::User(body) = &module.functions[lambda.definition.source_function()].kind
    else {
        panic!("lambda invoke must have a user body")
    };
    let names = local_names(body);
    assert!(names.contains(&"None"));
    assert!(names.contains(&"Ready"));
}

#[test]
fn unit_literals_are_refutable_but_an_explicit_unit_field_binding_is_valid() {
    for (target, init) in [
        (pat_lit(unit_lit()), unit_lit()),
        (
            pat_named_subpatterns(&[], vec![("Unit", pat_lit(unit_lit()))], None),
            struct_init("UnitBox", vec![unit_lit()]),
        ),
    ] {
        let source = file(vec![
            struct_decl("UnitBox", vec![("Unit", ty_named("Unit"))]),
            fun("main", vec![val_pat(false, target, None, init)]),
        ]);
        let diagnostics = lower_user(source).expect_err("Unit literal binding must be rejected");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }

    let source = file(vec![
        struct_decl("UnitBox", vec![("Unit", ty_named("Unit"))]),
        fun(
            "main",
            vec![
                val("box", struct_init("UnitBox", vec![unit_lit()])),
                val_pat(
                    false,
                    pat_named_subpatterns(&[], vec![("Unit", pat_bind("value"))], None),
                    None,
                    var("box"),
                ),
            ],
        ),
    ]);
    let module = lower_user(source).expect("an explicit Unit field rename is a binding");
    assert!(local_names(function_body(&module, "main")).contains(&"value"));
}

#[test]
fn bare_unit_variant_in_match_remains_variant_first() {
    let source = file(vec![
        signal_decl(),
        fun_sig(
            "check",
            vec![],
            vec![("signal", ty_named("Signal"))],
            None,
            vec![when_stmt(
                var("signal"),
                vec![arm(pat_bind("Ready"), None, vec![])],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);

    let module = lower_user(source).expect("bare unit variants remain match patterns");
    let when = function_body(&module, "check")
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("check contains a when");
    assert!(matches!(
        case_pattern(&when.arms[0]),
        hir::Pattern::Variant {
            application,
            fields,
        } if fields.is_empty()
            && module.enum_member_identities.variant_declaration(application.variant)
                .expect("the pattern retains its original variant").local_index() == 0
    ));
}

#[test]
fn bare_payload_variant_in_match_requires_an_explicit_shape() {
    let source = file(vec![
        signal_decl(),
        fun_sig(
            "check",
            vec![],
            vec![("signal", ty_named("Signal"))],
            None,
            vec![when_stmt(
                var("signal"),
                vec![arm(pat_bind("Payload"), None, vec![])],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);

    let diagnostics = lower_user(source).expect_err("payload variants need a pattern shape");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "variant `Payload` of `Signal` has 1 field(s); use `Payload(...)` to match it"
    );
}

#[test]
fn match_field_shorthand_classifies_its_same_named_subpattern_from_the_field_type() {
    #[derive(Clone, Copy)]
    enum Expected {
        UnitWildcard,
        UnitVariant,
        PayloadVariantError,
    }

    for (field_name, field_ty, shorthand, expected) in [
        (
            "Unit",
            ty_named("Unit"),
            pat_lit(unit_lit()),
            Expected::UnitWildcard,
        ),
        (
            "Ready",
            ty_named("Signal"),
            pat_bind("Ready"),
            Expected::UnitVariant,
        ),
        (
            "Payload",
            ty_named("Signal"),
            pat_bind("Payload"),
            Expected::PayloadVariantError,
        ),
    ] {
        let source = file(vec![
            signal_decl(),
            struct_decl("Envelope", vec![(field_name, field_ty)]),
            fun_sig(
                "check",
                vec![],
                vec![("envelope", ty_named("Envelope"))],
                None,
                vec![when_stmt(
                    var("envelope"),
                    vec![arm(
                        // The parser expands shorthand into the same-named
                        // subpattern while preserving `Unit` as a literal.
                        pat_named_subpatterns(&[], vec![(field_name, shorthand)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                )],
            ),
            fun("main", vec![]),
        ]);

        if matches!(expected, Expected::PayloadVariantError) {
            let diagnostics = lower_user(source)
                .expect_err("a payload variant used through shorthand still needs a shape");
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(
                diagnostics[0].message,
                "variant `Payload` of `Signal` has 1 field(s); use `Payload(...)` to match it"
            );
            continue;
        }

        let module = lower_user(source).expect("match shorthand must use match-context lookup");
        let when = function_body(&module, "check")
            .statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::StatementKind::When(when) => Some(when),
                _ => None,
            })
            .expect("check contains a when");
        let hir::Pattern::Struct { fields, .. } = case_pattern(&when.arms[0]) else {
            panic!("field shorthand must retain its enclosing struct pattern")
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, 0);

        match expected {
            Expected::UnitWildcard => assert!(matches!(&fields[0].1, hir::Pattern::Wildcard)),
            Expected::UnitVariant => assert!(matches!(
                &fields[0].1,
                hir::Pattern::Variant {
                    application,
                    fields,
                } if fields.is_empty()
                    && module.enum_member_identities.variant_declaration(application.variant)
                        .expect("the pattern retains its original variant").local_index() == 0
            )),
            Expected::PayloadVariantError => unreachable!(),
        }
    }
}

#[test]
fn explicit_enum_shapes_remain_refutable_in_binding_positions() {
    for target in [
        pat_pos(&["Signal", "Ready"], vec![], None),
        pat_pos(&["Ready"], vec![], None),
        pat_pos(&["Payload"], vec![pat_bind("value")], None),
    ] {
        let source = file(vec![
            signal_decl(),
            fun_sig(
                "reject",
                vec![],
                vec![("signal", ty_named("Signal"))],
                None,
                vec![val_pat(false, target, None, var("signal"))],
            ),
            fun("main", vec![]),
        ]);
        let diagnostics = lower_user(source).expect_err("enum shapes are refutable bindings");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }
}

#[test]
fn failed_binding_pattern_does_not_leak_earlier_leaf_bindings() {
    let source = file(vec![
        fun_sig(
            "consume",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![],
        ),
        fun(
            "main",
            vec![
                val("pair", tuple_lit(vec![int_lit(1), int_lit(2)])),
                val_pat(
                    false,
                    pat_tuple(vec![pat_bind("escaped"), pat_bind("escaped")], None),
                    None,
                    var("pair"),
                ),
                stmt(call("consume", vec![var("escaped")])),
            ],
        ),
    ]);

    let diagnostics = lower_user(source).expect_err("the invalid pattern must not commit locals");
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "`escaped` is already declared in this scope"
    );
    assert!(
        diagnostics[1]
            .message
            .starts_with("unknown variable `escaped`"),
        "{diagnostics:?}"
    );
}

#[test]
fn failed_match_pattern_does_not_commit_a_catch_all_warning() {
    let source = file(vec![
        signal_decl(),
        fun(
            "main",
            vec![
                val(
                    "subject",
                    tuple_lit(vec![field(var("Signal"), "Ready"), int_lit(1)]),
                ),
                when_stmt(
                    var("subject"),
                    vec![arm(
                        pat_tuple(
                            vec![pat_bind("Misspelled"), pat_lit(str_lit("wrong"))],
                            None,
                        ),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);

    let diagnostics = lower_user(source).expect_err("the second tuple column is ill-typed");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "literal pattern of type String cannot match Int"
    );
}
