use super::*;

fn function_when<'module>(module: &'module hir::Module, name: &str) -> &'module hir::When {
    let function = module
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == name)
        .expect("test function must exist");
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(value) => Some(value),
            _ => None,
        })
        .expect("test function must contain a when")
}

fn bool_pattern(value: bool) -> ast::Pattern {
    pat_lit(bool_lit(value))
}

#[test]
fn recursive_named_fields_lower_by_exact_type_in_declaration_order() {
    let payload_ty = ty_tuple(vec![ty_named("Leaf"), ty_named("Choice")]);
    let source = file(vec![
        struct_decl(
            "Leaf",
            vec![("flag", ty_named("Boolean")), ("count", ty_named("Int"))],
        ),
        enum_decl(
            "Choice",
            vec![],
            vec![
                variant_named("Present", vec![("value", ty_named("Boolean"))]),
                variant_unit("Absent"),
            ],
        ),
        generic_struct_decl(
            "Container",
            vec!["T"],
            vec![
                ("first", ty_named("Int")),
                ("payload", ty_named("T")),
                ("last", ty_named("String")),
            ],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_generic("Container", vec![payload_ty]))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![arm(
                    pat_named_subpatterns(
                        &[],
                        vec![
                            ("last", pat_wild()),
                            (
                                "payload",
                                pat_tuple(
                                    vec![
                                        pat_named_subpatterns(
                                            &[],
                                            vec![("flag", bool_pattern(false))],
                                            Some(sp()),
                                        ),
                                        pat_named_subpatterns(
                                            &["Present"],
                                            vec![("value", bool_pattern(true))],
                                            None,
                                        ),
                                    ],
                                    None,
                                ),
                            ),
                            ("first", pat_lit(int_lit(0))),
                        ],
                        None,
                    ),
                    None,
                    vec![],
                )],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(source).expect("recursive field patterns must lower");
    let hir::Pattern::Struct { fields, .. } = &function_when(&module, "check").arms[0].pattern
    else {
        panic!("outer pattern must resolve to a struct")
    };

    assert_eq!(
        fields.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(matches!(&fields[0].1, hir::Pattern::Literal { .. }));
    assert!(matches!(&fields[2].1, hir::Pattern::Wildcard));

    let hir::Pattern::Tuple(payload) = &fields[1].1 else {
        panic!("generic payload field must retain its exact tuple type")
    };
    let hir::Pattern::Struct {
        fields: leaf_fields,
        ..
    } = &payload[0]
    else {
        panic!("nested unprefixed field pattern must resolve to Leaf")
    };
    assert_eq!(
        leaf_fields
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(matches!(&leaf_fields[0].1, hir::Pattern::Literal { .. }));
    assert!(matches!(&leaf_fields[1].1, hir::Pattern::Wildcard));

    let hir::Pattern::Variant {
        application,
        fields: variant_fields,
    } = &payload[1]
    else {
        panic!("nested named variant must retain its variant shape")
    };
    assert_eq!(
        module
            .enum_member_identities
            .variant_declaration(application.variant)
            .expect("the nested pattern retains its original variant")
            .local_index(),
        0
    );
    assert_eq!(variant_fields.len(), 1);
    assert!(matches!(&variant_fields[0].1, hir::Pattern::Literal { .. }));
}

#[test]
fn generic_named_field_subpatterns_use_instantiated_field_types() {
    let source = file(vec![
        generic_struct_decl(
            "Box",
            vec!["T"],
            vec![("value", ty_named("T")), ("marker", ty_named("Boolean"))],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_generic("Box", vec![ty_named("String")]))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![arm(
                    pat_named_subpatterns(
                        &[],
                        vec![("value", pat_lit(str_lit("hit")))],
                        Some(sp()),
                    ),
                    None,
                    vec![],
                )],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(source).expect("generic field type must be instantiated");
    let hir::Pattern::Struct { fields, .. } = &function_when(&module, "check").arms[0].pattern
    else {
        panic!("Box field pattern must resolve to a struct")
    };
    assert_eq!(fields.len(), 2);
    let hir::Pattern::Literal { subject_ty, .. } = &fields[0].1 else {
        panic!("value field must retain its String literal pattern")
    };
    assert!(matches!(module.types[*subject_ty], hir::Type::String));
    assert!(matches!(&fields[1].1, hir::Pattern::Wildcard));
}

#[test]
fn lambda_parameters_share_recursive_irrefutable_named_lowering() {
    let parameter_pattern = pat_named_subpatterns(
        &[],
        vec![(
            "leaf",
            pat_named_subpatterns(
                &[],
                vec![("value", pat_bind("value")), ("flag", pat_wild())],
                None,
            ),
        )],
        None,
    );
    let operation = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: parameter_pattern,
            ty: None,
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    let source = file(vec![
        struct_decl(
            "Leaf",
            vec![("value", ty_named("Int")), ("flag", ty_named("Boolean"))],
        ),
        struct_decl("Root", vec![("leaf", ty_named("Leaf"))]),
        fun(
            "main",
            vec![val_ty(
                "extract",
                Some(ty_function(false, vec![ty_named("Root")], ty_named("Int"))),
                operation,
            )],
        ),
    ]);
    let module = lower_user(source).expect("nested named lambda parameter must lower");
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    let invoke = &module.functions[lambda.definition.source_function()];
    assert_eq!(
        invoke.params.len(),
        2,
        "closure receiver plus one complete source parameter"
    );
    assert_eq!(invoke.params[1].name, "$arg.0");
    let hir::FunctionKind::User(body) = &invoke.kind else {
        panic!("lambda invoke must have a body")
    };
    assert!(body.statements[..3].iter().all(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { .. },
            ..
        }
    )));
    let projections = body.statements[..3]
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            let hir::ExprKind::FieldAccess {
                field: hir::FieldRef::StructField { field, .. },
                ..
            } = init.kind
            else {
                return None;
            };
            Some(
                module
                    .field_identities
                    .struct_declaration(field)
                    .unwrap()
                    .local_index(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(projections, vec![0, 0]);
    assert_eq!(body.locals[invoke.params[1].local].name, "$arg.0");
    assert!(body.locals.iter().any(|(_, local)| local.name == "value"));
    assert!(
        !body
            .locals
            .iter()
            .any(|(_, local)| local.name.starts_with("$binding.subject."))
    );
}

#[test]
fn deeply_refutable_named_subpatterns_are_rejected_in_bindings() {
    let cases = [
        pat_named_subpatterns(
            &[],
            vec![(
                "leaf",
                pat_named_subpatterns(&[], vec![("flag", bool_pattern(false))], Some(sp())),
            )],
            None,
        ),
        pat_named_subpatterns(
            &[],
            vec![(
                "leaf",
                pat_named_subpatterns(
                    &[],
                    vec![(
                        "choice",
                        pat_named_subpatterns(&["Present"], vec![("value", pat_wild())], None),
                    )],
                    Some(sp()),
                ),
            )],
            None,
        ),
    ];

    for target in cases {
        let source = file(vec![
            enum_decl(
                "Choice",
                vec![],
                vec![
                    variant_named("Present", vec![("value", ty_named("Int"))]),
                    variant_unit("Absent"),
                ],
            ),
            struct_decl(
                "Leaf",
                vec![
                    ("flag", ty_named("Boolean")),
                    ("choice", ty_named("Choice")),
                ],
            ),
            struct_decl("Root", vec![("leaf", ty_named("Leaf"))]),
            fun_sig(
                "reject",
                vec![],
                vec![("subject", ty_named("Root"))],
                None,
                vec![val_pat(false, target, None, var("subject"))],
            ),
            fun("main", vec![]),
        ]);
        let diagnostics = lower_user(source).expect_err("deep refutable binding must fail");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }
}

#[test]
fn named_field_rows_prove_exhaustiveness_and_report_recursive_witnesses() {
    let gate = || {
        enum_decl(
            "Gate",
            vec![],
            vec![variant_named("Value", vec![("flag", ty_named("Boolean"))])],
        )
    };
    let value_pattern =
        |value| pat_named_subpatterns(&["Value"], vec![("flag", bool_pattern(value))], None);
    let complete = file(vec![
        gate(),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_named("Gate"))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![
                    arm(value_pattern(false), None, vec![]),
                    arm(value_pattern(true), None, vec![]),
                ],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(complete).expect("both named Boolean rows are exhaustive");
    assert!(matches!(
        &function_when(&module, "check").fallback,
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix { .. })
    ));

    let missing = file(vec![
        gate(),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_named("Gate"))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![arm(value_pattern(false), None, vec![])],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let diagnostics = lower_user(missing).expect_err("true payload remains uncovered");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "non-exhaustive when: missing pattern Value { flag: true }"
    );
}

#[test]
fn recursive_named_fields_preserve_binding_and_type_diagnostics() {
    let duplicate = pat_named_subpatterns(
        &[],
        vec![
            (
                "left",
                pat_named_subpatterns(&[], vec![("value", pat_bind("same"))], None),
            ),
            (
                "right",
                pat_named_subpatterns(&[], vec![("value", pat_bind("same"))], None),
            ),
        ],
        None,
    );
    let source = file(vec![
        struct_decl("Leaf", vec![("value", ty_named("String"))]),
        struct_decl(
            "Pair",
            vec![("left", ty_named("Leaf")), ("right", ty_named("Leaf"))],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_named("Pair"))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![arm(duplicate, None, vec![])],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let diagnostics = lower_user(source).expect_err("deep duplicate binding must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "`same` is already declared in this scope"
    );

    let mismatch = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        fun_sig(
            "check",
            vec![],
            vec![("subject", ty_generic("Box", vec![ty_named("String")]))],
            None,
            vec![when_stmt(
                var("subject"),
                vec![arm(
                    pat_named_subpatterns(&[], vec![("value", pat_lit(int_lit(1)))], None),
                    None,
                    vec![],
                )],
                Some(vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let diagnostics = lower_user(mismatch).expect_err("generic field mismatch must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "literal pattern of type Int cannot match String"
    );
}
