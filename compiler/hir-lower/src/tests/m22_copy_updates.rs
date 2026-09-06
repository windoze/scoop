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
fn copy_target_diagnostics_are_decided_before_any_rhs_is_committed() {
    let cases = [
        (
            file(vec![fun(
                "main",
                vec![val("bad", copy_update(int_lit(1), vec![("x", int_lit(2))]))],
            )]),
            "copy update requires an exact declared struct value, found Int",
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
fn struct_copy_rhs_must_match_the_exact_field_type() {
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
fn every_enum_shape_is_rejected_before_field_or_rhs_resolution() {
    let cases = [
        (
            file(vec![
                enum_decl("UnitChoice", Vec::new(), vec![variant_unit("Only")]),
                fun(
                    "main",
                    vec![
                        val_ty("base", Some(ty_named("UnitChoice")), var("Only")),
                        val(
                            "bad",
                            copy_update(var("base"), vec![("x", call("unknown", vec![]))]),
                        ),
                    ],
                ),
            ]),
            "UnitChoice",
        ),
        (
            file(vec![
                enum_decl(
                    "PositionalChoice",
                    Vec::new(),
                    vec![variant_positional("Item", vec![ty_named("Int")])],
                ),
                fun(
                    "main",
                    vec![
                        val_ty(
                            "base",
                            Some(ty_named("PositionalChoice")),
                            call("Item", vec![int_lit(1)]),
                        ),
                        val(
                            "bad",
                            copy_update(var("base"), vec![("x", call("unknown", vec![]))]),
                        ),
                    ],
                ),
            ]),
            "PositionalChoice",
        ),
        (
            file(vec![
                enum_decl(
                    "NamedChoice",
                    Vec::new(),
                    vec![variant_named("Data", vec![("x", ty_named("Int"))])],
                ),
                fun(
                    "main",
                    vec![
                        val_ty(
                            "base",
                            Some(ty_named("NamedChoice")),
                            source_call("Data", vec![named_argument("x", int_lit(1))]),
                        ),
                        val(
                            "bad",
                            copy_update(var("base"), vec![("x", call("unknown", vec![]))]),
                        ),
                    ],
                ),
            ]),
            "NamedChoice",
        ),
        (
            file(vec![
                enum_decl(
                    "GenericChoice",
                    vec!["T"],
                    vec![variant_named("Data", vec![("value", ty_named("T"))])],
                ),
                fun_expr(
                    "reject",
                    Vec::new(),
                    vec![(
                        "base",
                        ty_generic("GenericChoice", vec![ty_named("String")]),
                    )],
                    Some(ty_generic("GenericChoice", vec![ty_named("String")])),
                    copy_update(var("base"), vec![("value", call("unknown", vec![]))]),
                ),
                fun("main", Vec::new()),
            ]),
            "GenericChoice<String>",
        ),
    ];

    for (source, ty) in cases {
        let diagnostics = lower_user(source).expect_err("enum copy update must be rejected");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].message,
            format!("copy update requires an exact declared struct value, found {ty}")
        );
        assert_ne!(diagnostics[0].message, "unknown function `unknown`");
    }
}

#[test]
fn generic_copy_nodes_dump_their_exact_applied_struct_identity() {
    let module = lower_user(file(vec![
        generic_struct_decl(
            "Box",
            vec!["T"],
            vec![("value", ty_named("T")), ("marker", ty_named("Int"))],
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
        fun("main", Vec::new()),
    ]))
    .expect("generic copy-update applications must stay exact");

    let dump = hir::dump(&module);
    for expected in [
        "StructConstruct Box<Int> : Box<Int>",
        "StructConstruct Box<String> : Box<String>",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
}
