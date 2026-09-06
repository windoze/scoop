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

fn hidden_initializers<'body>(body: &'body hir::Body, prefix: &str) -> Vec<&'body hir::Expr> {
    body.statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let hir::Pattern::Binding { local } = pattern else {
                return None;
            };
            body.locals[*local].name.starts_with(prefix).then_some(init)
        })
        .collect()
}

#[test]
fn struct_copy_materializes_base_and_rhs_then_reconstructs_in_declaration_order() {
    let module = lower_user(file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("label", ty_named("String"))],
        ),
        fun_expr(
            "nextX",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun_expr(
            "nextLabel",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            str_lit("next"),
        ),
        fun(
            "main",
            vec![
                val(
                    "point",
                    struct_init("Point", vec![int_lit(1), str_lit("old")]),
                ),
                val(
                    "moved",
                    copy_update(
                        var("point"),
                        vec![
                            ("label", call("nextLabel", vec![])),
                            ("x", call("nextX", vec![])),
                        ],
                    ),
                ),
            ],
        ),
    ]))
    .expect("declared struct copy update must lower");
    let body = function_body(&module, "main");
    let bases = hidden_initializers(body, "$copy.base");
    assert_eq!(bases.len(), 1);
    assert!(matches!(bases[0].kind, hir::ExprKind::Local(_)));
    let updates = hidden_initializers(body, "$copy.update");
    assert_eq!(updates.len(), 2);
    let callees = updates
        .iter()
        .map(|expression| match expression.kind {
            hir::ExprKind::Call {
                callee: hir::Callable::Function(function),
                ..
            } => module.functions[function].name.as_str(),
            ref other => panic!("copy RHS must initialize from a call, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(callees, ["nextLabel", "nextX"]);

    let moved = local_init(body, "moved");
    let hir::ExprKind::StructConstruct {
        application,
        fields,
    } = &moved.kind
    else {
        panic!("copy result must be a raw struct construction");
    };
    assert_eq!(
        module.struct_applications[*application].canonical_type,
        moved.ty
    );
    assert_eq!(fields.len(), 2);
    let field_locals = fields
        .iter()
        .map(|field| match field.kind {
            hir::ExprKind::Local(local) => body.locals[local].name.as_str(),
            ref other => panic!("updated field must read its temporary, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert!(field_locals[0].starts_with("$copy.update"));
    assert!(field_locals[1].starts_with("$copy.update"));
    assert_ne!(field_locals[0], field_locals[1]);
}

#[test]
fn enum_copy_guards_rhs_and_projects_only_on_the_matching_true_edge() {
    let module = lower_user(file(vec![
        enum_decl(
            "Event",
            Vec::new(),
            vec![
                variant_named(
                    "Data",
                    vec![("code", ty_named("Int")), ("text", ty_named("String"))],
                ),
                variant_named("Other", vec![("code", ty_named("Int"))]),
            ],
        ),
        fun_expr(
            "replacement",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            str_lit("new"),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "event",
                    Some(ty_named("Event")),
                    source_call(
                        "Event.Data",
                        vec![
                            named_argument("code", int_lit(1)),
                            named_argument("text", str_lit("old")),
                        ],
                    ),
                ),
                val(
                    "changed",
                    copy_update(var("event"), vec![("text", call("replacement", vec![]))]),
                ),
            ],
        ),
    ]))
    .expect("a uniquely selected enum copy update must lower");
    let body = function_body(&module, "main");
    let statement = body
        .statements
        .iter()
        .find(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
        .expect("enum copy emits a guard");
    let hir::StatementKind::If {
        cond,
        then_body,
        else_body: Some(else_body),
    } = &statement.kind
    else {
        unreachable!("selected statement is the complete copy guard")
    };
    let hir::ExprKind::VariantTest { operand, variant } = &cond.kind else {
        panic!("copy guard must be a typed variant test")
    };
    let hir::ExprKind::Local(tested_local) = operand.kind else {
        panic!("copy guard operand must be the stable base local")
    };
    assert_eq!(
        module.enums[variant.declaration().enumeration()].name,
        "Event"
    );
    assert_eq!(variant.local_index(), 0);
    let rhs_position = then_body
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                hir::StatementKind::ValDecl { init, .. }
                    if matches!(init.kind, hir::ExprKind::Call { .. })
            )
        })
        .expect("the source RHS is materialized on the true edge");
    let projection_locals = then_body[..rhs_position]
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let hir::Pattern::Binding { local } = pattern else {
                return None;
            };
            let hir::ExprKind::VariantPayloadProject { operand, field } = &init.kind else {
                return None;
            };
            assert_eq!(field.variant(), *variant);
            assert!(matches!(operand.kind, hir::ExprKind::Local(local) if local == tested_local));
            Some(*local)
        })
        .collect::<Vec<_>>();
    assert_eq!(projection_locals.len(), 2);
    let assigned = then_body
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::Assign { value, .. } => Some(value),
            _ => None,
        })
        .expect("the true edge assigns the reconstructed result");
    let hir::ExprKind::VariantConstruct {
        variant: rebuilt,
        args,
    } = &assigned.kind
    else {
        panic!("true edge must raw-rebuild the same variant")
    };
    assert_eq!(rebuilt, variant);
    let hir::ExprKind::Local(preserved) = args[0].kind else {
        panic!("untouched enum field must use its pre-RHS payload temporary")
    };
    assert!(projection_locals.contains(&preserved));
    assert!(matches!(
        &else_body[0].kind,
        hir::StatementKind::Throw(hir::Expr {
            kind: hir::ExprKind::ClassInit { args, .. },
            ..
        }) if matches!(
            args.as_slice(),
            [hir::Expr {
                kind: hir::ExprKind::SomeWrap(message),
                ..
            }] if matches!(
                message.kind,
                hir::ExprKind::StringLiteral(ref text)
                    if text == "copy update expected enum `Event` variant `Data`"
            )
        )
    ));

    let concrete = crate::concretize::lower(&module);
    let main = concrete
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "main").then_some(function))
        .expect("concrete main exists");
    let hir::concrete::FunctionKind::User(body) = &main.kind else {
        panic!("concrete main has a user body")
    };
    let hir::concrete::StatementKind::If {
        cond, then_body, ..
    } = body
        .statements
        .iter()
        .find_map(|statement| {
            matches!(statement.kind, hir::concrete::StatementKind::If { .. })
                .then_some(&statement.kind)
        })
        .expect("concretization preserves the guard")
    else {
        unreachable!("selected concrete statement is an if")
    };
    assert!(matches!(
        cond.kind,
        hir::concrete::ExprKind::VariantTest { .. }
    ));
    assert!(then_body.iter().any(|statement| matches!(
        statement.kind,
        hir::concrete::StatementKind::Assign {
            value: hir::concrete::Expr {
                kind: hir::concrete::ExprKind::VariantConstruct { .. },
                ..
            },
            ..
        }
    )));
}

#[test]
fn copy_target_diagnostics_are_decided_before_any_rhs_is_committed() {
    let cases = [
        (
            file(vec![fun(
                "main",
                vec![val("bad", copy_update(int_lit(1), vec![("x", int_lit(2))]))],
            )]),
            "copy update requires an exact declared struct or enum value, found Int",
        ),
        (
            file(vec![
                struct_decl("S", vec![("x", ty_named("Int"))]),
                fun(
                    "main",
                    vec![
                        val("s", struct_init("S", vec![int_lit(1)])),
                        val(
                            "bad",
                            copy_update(var("s"), vec![("missing", call("unknown", vec![]))]),
                        ),
                    ],
                ),
            ]),
            "struct `S` has no field `missing`",
        ),
        (
            file(vec![
                struct_decl("S", vec![("x", ty_named("Int"))]),
                fun(
                    "main",
                    vec![
                        val("s", struct_init("S", vec![int_lit(1)])),
                        val(
                            "bad",
                            copy_update(var("s"), vec![("x", int_lit(2)), ("x", int_lit(3))]),
                        ),
                    ],
                ),
            ]),
            "copy update field `x` is specified more than once",
        ),
    ];
    for (source, expected) in cases {
        let errors = lower_user(source).expect_err("invalid copy target must be rejected");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].message, expected);
        assert!(
            errors[0].message != "unknown function `unknown`",
            "an RHS must not be lowered before its target exists"
        );
    }
}

#[test]
fn enum_copy_rejects_zero_multiple_and_wrong_typed_targets() {
    let zero = lower_user(file(vec![
        enum_decl(
            "Choice",
            Vec::new(),
            vec![variant_named("A", vec![("x", ty_named("Int"))])],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "c",
                    Some(ty_named("Choice")),
                    source_call("Choice.A", vec![named_argument("x", int_lit(1))]),
                ),
                val("bad", copy_update(var("c"), vec![("y", int_lit(2))])),
            ],
        ),
    ]))
    .expect_err("a missing common enum field must fail");
    assert_eq!(
        zero[0].message,
        "enum `Choice` has no named-payload variant containing all copy-update fields: `y`"
    );

    let multiple = lower_user(file(vec![
        enum_decl(
            "Choice",
            Vec::new(),
            vec![
                variant_named("A", vec![("x", ty_named("Int"))]),
                variant_constructor("B", vec![("x", ty_named("Int"), Some(int_lit(0)))]),
            ],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "c",
                    Some(ty_named("Choice")),
                    source_call("Choice.A", vec![named_argument("x", int_lit(1))]),
                ),
                val("bad", copy_update(var("c"), vec![("x", int_lit(2))])),
            ],
        ),
    ]))
    .expect_err("two named field candidates must be ambiguous");
    assert_eq!(
        multiple[0].message,
        "copy update of enum `Choice` is ambiguous between variants `A`, `B`; use `when` to rebuild the intended variant explicitly"
    );

    let mismatch = lower_user(file(vec![
        struct_decl("S", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("s", struct_init("S", vec![int_lit(1)])),
                val("bad", copy_update(var("s"), vec![("x", str_lit("wrong"))])),
            ],
        ),
    ]))
    .expect_err("copy RHS must match the exact field type");
    assert_eq!(
        mismatch[0].message,
        "copy update field `x` expects Int, found String"
    );
}

#[test]
fn failed_contextual_copy_probe_does_not_leave_ghost_locals_or_variant_calls() {
    let module = lower_user(file(vec![
        enum_decl(
            "Small",
            Vec::new(),
            vec![variant_constructor("V", vec![("y", ty_named("Int"), None)])],
        ),
        enum_decl(
            "Wide",
            Vec::new(),
            vec![variant_constructor(
                "V",
                vec![
                    ("y", ty_named("Int"), None),
                    ("x", ty_named("Int"), Some(int_lit(0))),
                ],
            )],
        ),
        fun_expr(
            "take",
            Vec::new(),
            vec![("value", ty_named("Small"))],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun_expr(
            "take",
            Vec::new(),
            vec![("value", ty_named("Wide"))],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun(
            "main",
            vec![val(
                "result",
                call(
                    "take",
                    vec![copy_update(
                        source_call("V", vec![named_argument("y", int_lit(1))]),
                        vec![("x", int_lit(2))],
                    )],
                ),
            )],
        ),
    ]))
    .expect("the applicable contextual enum copy candidate must win");
    let body = function_body(&module, "main");
    let dump = hir::dump(&module);
    assert!(!dump.contains("VariantConstruct Small.V"), "{dump}");
    assert_eq!(
        body.locals
            .iter()
            .filter(|(_, local)| local.name.starts_with("$copy.base"))
            .count(),
        1
    );
    assert_eq!(
        body.locals
            .iter()
            .filter(|(_, local)| local.name.starts_with("$copy.update"))
            .count(),
        1
    );
}

#[test]
fn generic_copy_nodes_dump_their_exact_applied_struct_and_enum_identity() {
    let module = lower_user(file(vec![
        generic_struct_decl(
            "Box",
            vec!["T"],
            vec![("value", ty_named("T")), ("marker", ty_named("Int"))],
        ),
        enum_decl(
            "Choice",
            vec!["T"],
            vec![
                variant_named(
                    "Data",
                    vec![("value", ty_named("T")), ("marker", ty_named("Int"))],
                ),
                variant_unit("Empty"),
            ],
        ),
        fun_expr(
            "structInt",
            Vec::new(),
            vec![("base", ty_generic("Box", vec![ty_named("Int")]))],
            Some(ty_generic("Box", vec![ty_named("Int")])),
            copy_update(var("base"), vec![("marker", int_lit(1))]),
        ),
        fun_expr(
            "structString",
            Vec::new(),
            vec![("base", ty_generic("Box", vec![ty_named("String")]))],
            Some(ty_generic("Box", vec![ty_named("String")])),
            copy_update(var("base"), vec![("marker", int_lit(2))]),
        ),
        fun_expr(
            "enumInt",
            Vec::new(),
            vec![("base", ty_generic("Choice", vec![ty_named("Int")]))],
            Some(ty_generic("Choice", vec![ty_named("Int")])),
            copy_update(var("base"), vec![("marker", int_lit(3))]),
        ),
        fun_expr(
            "enumString",
            Vec::new(),
            vec![("base", ty_generic("Choice", vec![ty_named("String")]))],
            Some(ty_generic("Choice", vec![ty_named("String")])),
            copy_update(var("base"), vec![("marker", int_lit(4))]),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("generic copy-update applications must stay exact");

    let dump = hir::dump(&module);
    for expected in [
        "StructConstruct Box<Int> : Box<Int>",
        "StructConstruct Box<String> : Box<String>",
        "VariantTest Choice<Int>.Data : Boolean",
        "VariantTest Choice<String>.Data : Boolean",
        "VariantPayloadProject Choice<Int>.Data.value : Int",
        "VariantPayloadProject Choice<String>.Data.value : String",
        "VariantConstruct Choice.Data<Int> : Choice<Int>",
        "VariantConstruct Choice.Data<String> : Choice<String>",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
}
