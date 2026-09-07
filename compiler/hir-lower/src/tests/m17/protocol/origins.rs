use super::*;

#[test]
fn concrete_default_origins_keep_definition_and_outermost_evaluation_sites() {
    let inner_definition = ast::Span::new(10, 12);
    let nested_call_definition = ast::Span::new(20, 25);
    let outer_call_site = ast::Span::new(100, 107);
    let inner = with_default(
        fun_sig(
            "inner",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        Expr::IntLiteral(ast::IntegerLiteralSyntax {
            span: inner_definition,
            ..integer_syntax(41)
        }),
    );
    let outer = with_default(
        fun_sig(
            "outer",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        call_with_span("inner", nested_call_definition),
    );
    let output = lower_user_output(file(vec![
        inner,
        outer,
        fun(
            "main",
            vec![val("result", call_with_span("outer", outer_call_site))],
        ),
    ]))
    .expect("nested defaults must preserve complete expression origins");

    assert!(
        output
            .export
            .export_default_exprs
            .iter()
            .all(|(_, template)| {
                matches!(template.value.origin, hir::ExpressionOrigin::Definition(_))
                    && template.statements.iter().all(|statement| {
                        !matches!(
                            &statement.kind,
                            hir::StatementKind::ValDecl {
                                init: hir::Expr {
                                    origin: hir::ExpressionOrigin::Instantiated(_),
                                    ..
                                },
                                ..
                            }
                        )
                    })
            })
    );

    let main = concrete_function_body(&output.local, "main");
    let origin =
        main.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::concrete::StatementKind::ValDecl {
                    init:
                        hir::concrete::Expr {
                            kind:
                                hir::concrete::ExprKind::IntegerLiteral(
                                    hir::HirIntegerConstant::Signed32(41),
                                ),
                            origin,
                            ..
                        },
                    ..
                } => Some(*origin),
                _ => None,
            })
            .expect("instantiated inner default literal");
    assert_eq!(origin.definition.span, inner_definition);
    assert_eq!(origin.evaluation.span, outer_call_site);
}

#[test]
fn a_lambda_body_establishes_its_own_default_evaluation_boundary() {
    let inner_definition = ast::Span::new(10, 12);
    let lambda_call_site = ast::Span::new(40, 47);
    let factory_call_site = ast::Span::new(100, 109);
    let inner = with_default(
        fun_sig(
            "inner",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        Expr::IntLiteral(ast::IntegerLiteralSyntax {
            span: inner_definition,
            ..integer_syntax(42)
        }),
    );
    let callback_type = ty_function(false, vec![], ty_named("Int"));
    let factory = with_default(
        fun_sig(
            "factory",
            vec![],
            vec![("callback", callback_type.clone())],
            Some(callback_type),
            vec![ret(Some(var("callback")))],
        ),
        0,
        Expr::Lambda {
            id: ast::LambdaId(0),
            is_suspend: false,
            parameters: Some(Vec::new()),
            body: block(vec![stmt(call_with_span("inner", lambda_call_site))]),
            span: ast::Span::new(30, 50),
        },
    );
    let output = lower_user_output(file(vec![
        inner,
        factory,
        fun(
            "main",
            vec![val(
                "callback",
                call_with_span("factory", factory_call_site),
            )],
        ),
    ]))
    .expect("default-instantiated lambdas must retain their body origin boundary");

    let (_, lambda) = output
        .local
        .functions
        .iter()
        .find(|(_, function)| function.name.starts_with("$lambda"))
        .expect("lambda body");
    let hir::concrete::FunctionKind::User(body) = &lambda.kind else {
        unreachable!()
    };
    let origin =
        body.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::concrete::StatementKind::ValDecl {
                    init:
                        hir::concrete::Expr {
                            kind:
                                hir::concrete::ExprKind::IntegerLiteral(
                                    hir::HirIntegerConstant::Signed32(42),
                                ),
                            origin,
                            ..
                        },
                    ..
                } => Some(*origin),
                _ => None,
            })
            .expect("inner default in lambda body");
    assert_eq!(origin.definition.span, inner_definition);
    assert_eq!(origin.evaluation.span, lambda_call_site);
    assert_ne!(origin.evaluation.span, factory_call_site);
}

#[test]
fn current_source_location_reads_the_concrete_evaluation_origin() {
    let source = concat!(
        "fun trace(location: SourceLocation = getCurrentSourceLocation()): SourceLocation = location\n",
        "fun main() {\n",
        "    val direct = getCurrentSourceLocation()\n",
        "    val forwarded = trace()\n",
        "}\n",
    );
    let span_at = |needle: &str, after: usize| {
        let offset = source[after..].find(needle).expect("source marker") + after;
        ast::Span::new(offset as u32, (offset + needle.len()) as u32)
    };
    let default_span = span_at("getCurrentSourceLocation()", 0);
    let direct_span = span_at("getCurrentSourceLocation()", default_span.end as usize);
    let forwarded_span = span_at("trace()", direct_span.end as usize);
    let trace = with_default(
        fun_expr(
            "trace",
            vec![],
            vec![("location", ty_named("SourceLocation"))],
            Some(ty_named("SourceLocation")),
            var("location"),
        ),
        0,
        call_with_span("getCurrentSourceLocation", default_span),
    );
    let user = file(vec![
        trace,
        fun(
            "main",
            vec![
                val(
                    "direct",
                    call_with_span("getCurrentSourceLocation", direct_span),
                ),
                val("forwarded", call_with_span("trace", forwarded_span)),
            ],
        ),
    ]);
    let core = core_file();
    let core_provider = hir::IntrinsicProviderId::from_raw(0);
    let user_provider = hir::IntrinsicProviderId::from_raw(1);
    let output = lower_compilation_unit(
        &CompilationUnit {
            core: vec![ProviderSource {
                source: &core,
                provider: core_provider,
                name: "scoop.core",
                source_text: "",
            }],
            user: ProviderSource {
                source: &user,
                provider: user_provider,
                name: "app.scoop",
                source_text: source,
            },
        },
        IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("source locations should fold during ordinary concretization");

    let main = concrete_function_body(&output.local, "main");
    let locations: Vec<_> = main
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::ValDecl { init, .. } => location_fields(init),
            _ => None,
        })
        .collect();
    assert!(
        locations.contains(&("app.scoop", 3, 18, "main", "")),
        "locations: {locations:?}"
    );
    assert!(
        locations.contains(&("app.scoop", 4, 21, "main", "")),
        "locations: {locations:?}"
    );
}

fn location_fields(expr: &hir::concrete::Expr) -> Option<(&str, i64, i64, &str, &str)> {
    let hir::concrete::ExprKind::StructInit { args, .. } = &expr.kind else {
        return None;
    };
    let [file, line, column, function_name, type_name] = args.as_slice() else {
        return None;
    };
    let (
        hir::concrete::ExprKind::StringLiteral(file),
        hir::concrete::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(line)),
        hir::concrete::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(column)),
        hir::concrete::ExprKind::StringLiteral(function_name),
        hir::concrete::ExprKind::StringLiteral(type_name),
    ) = (
        &file.kind,
        &line.kind,
        &column.kind,
        &function_name.kind,
        &type_name.kind,
    )
    else {
        return None;
    };
    Some((file, *line as i64, *column as i64, function_name, type_name))
}

#[test]
fn source_location_context_distinguishes_member_generic_and_suspend_bodies() {
    let member = class_decl(
        ast::ClassModifier::Final,
        "Recorder",
        vec![],
        None,
        vec![],
        vec![method_expr(
            "locate",
            vec![],
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", vec![]),
        )],
    );
    let mut suspend_location = fun_sig(
        "suspendLocation",
        vec![],
        vec![],
        Some(ty_named("SourceLocation")),
        vec![ret(Some(call("getCurrentSourceLocation", vec![])))],
    );
    let Decl::Function(suspend_location_decl) = &mut suspend_location else {
        unreachable!()
    };
    suspend_location_decl.is_suspend = true;
    let output = lower_user_output(file(vec![
        member,
        fun_expr(
            "genericLocation",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", vec![]),
        ),
        suspend_location,
        fun(
            "main",
            vec![
                val("recorder", call("Recorder", vec![])),
                val("member", method_call(var("recorder"), "locate", vec![])),
                val("generic", call("genericLocation", vec![int_lit(1)])),
            ],
        ),
    ]))
    .expect("source location context is available in every callable shape");

    let context_for = |name: &str| {
        let (_, function) = output
            .local
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap_or_else(|| panic!("missing concrete function `{name}`"));
        let hir::concrete::FunctionKind::User(body) = &function.kind else {
            unreachable!()
        };
        body.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::concrete::StatementKind::Return { value: Some(value) } => {
                    location_fields(value)
                }
                hir::concrete::StatementKind::ValDecl { init, .. } => location_fields(init),
                _ => None,
            })
            .map(|(_, _, _, function, ty)| (function.to_string(), ty.to_string()))
            .unwrap_or_else(|| panic!("missing source location in `{name}`"))
    };
    assert_eq!(
        context_for("Recorder.locate"),
        ("locate".to_string(), "Recorder".to_string())
    );
    assert_eq!(
        context_for("genericLocation"),
        ("genericLocation".to_string(), String::new())
    );
    assert_eq!(
        context_for("suspendLocation"),
        ("suspendLocation".to_string(), String::new())
    );
}
