use super::*;

fn qualified(parts: &[&str]) -> ast::QualifiedNameSyntax {
    let (first, rest) = parts
        .split_first()
        .expect("test qualified names are nonempty");
    ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&[name]),
        span: sp(),
    };
    source
}

fn exact(package: &str, name: &str, alias: Option<&str>) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(&[package, name]),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn star(package: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Star {
        exposure: ast::ImportExposureSyntax::Local,
        namespace: qualified(&[package]),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: sp(),
    }
}

fn lower_sources(
    sources: Vec<ast::SourceFile>,
    core: ast::SourceFile,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let parsed = identified_test_sources(sources);
    let input = DefinedTestSources::try_new(
        vec![ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
}

fn core_with(declarations: Vec<Decl>) -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.extend(declarations);
    make_core_public(&mut core);
    core
}

fn extension(receiver: &str, name: &str, parameter: &str, marker: i64) -> Decl {
    extension_expr(
        ty_named(receiver),
        name,
        Vec::new(),
        vec![("value", ty_named(parameter))],
        Some(ty_named("Int")),
        int_lit(marker),
    )
}

fn operator(mut declaration: Decl) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test operator declaration is a function")
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    declaration
}

fn property(name: &str, receiver: Option<&str>, value_ty: &str) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: receiver.map(ty_named),
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named(value_ty),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(call(value_ty, Vec::new()))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    }
}

fn extension_property(receiver: &str, name: &str, value_ty: &str) -> Decl {
    Decl::Global(property(name, Some(receiver), value_ty))
}

fn owner_with_member_property(name: &str, value_ty: &str) -> Decl {
    let Decl::Struct(mut owner) = struct_decl(name, Vec::new()) else {
        panic!("the owner builder returns a struct")
    };
    owner
        .members
        .push(ast::StructMember::Property(Box::new(property(
            "route", None, value_ty,
        ))));
    Decl::Struct(owner)
}

fn invoke(receiver: &str, marker: i64) -> Decl {
    operator(extension(receiver, "invoke", "Boolean", marker))
}

fn consumer(expression: Expr) -> ast::SourceFile {
    file(vec![fun("main", vec![val("chosen", expression)])])
}

fn explicit_call(name: &str) -> Expr {
    method_call(call("Owner", Vec::new()), name, vec![bool_lit(true)])
}

fn marked_explicit_call(name: &str) -> Expr {
    let span = ast::Span::new(700, 710);
    Expr::MethodCall {
        receiver: Box::new(call("Owner", Vec::new())),
        name: ast::Ident {
            text: name.to_string(),
            span,
        },
        navigation: ast::Navigation::Direct,
        type_args: Vec::new(),
        args: call_arguments(vec![bool_lit(true)]),
        span,
    }
}

fn body<'a>(module: &'a hir::Module, name: &str) -> &'a hir::Body {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("test function `{name}` exists"));
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function `{name}` has a body")
    };
    body
}

fn chosen(output: &hir::Output) -> &hir::Expr {
    local_init(body(&output.export, "main"), "chosen")
}

fn assert_marker(module: &hir::Module, expression: &hir::Expr, expected: i64) {
    let callable = match &expression.kind {
        hir::ExprKind::Call {
            callee: hir::CallableTarget::Local(callee),
            ..
        } => *callee,
        hir::ExprKind::MethodCall {
            callee: hir::MethodCallee::Callable(hir::CallableTarget::Local(callee)),
            ..
        } => *callee,
        other => panic!("expected a resolved callable expression, found {other:?}"),
    };
    let function = &module.functions[module.callable_function(callable)];
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("the selected marker function has a source body")
    };
    assert!(
        matches!(
            return_value(&body.statements).kind,
            hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(value))
                if i64::from(value) == expected
        ),
        "selected the wrong marker body for `{}`",
        function.name
    );
}

#[test]
fn local_concrete_extension_retains_its_logical_receiver_in_mir_metadata() {
    let output = lower_sources(
        vec![file(vec![extension("Int", "bump", "Int", 1)])],
        core_file(),
    )
    .expect("the extension source lowers");
    let (_, function) = output
        .local
        .functions
        .iter()
        .find(|(_, function)| function.name == "bump")
        .expect("the concrete extension is materialized");
    let hir::concrete::FunctionReceiver::Extension(receiver) = function.receiver else {
        panic!("the concrete function must retain its extension receiver")
    };
    assert_eq!(function.params[0].ty, receiver);
    let exact_receiver = output.local.exact_type_identities[receiver].id();
    let exact_parameter = output.local.exact_type_identities[function.params[1].ty].id();
    let materialization = function.materialization;

    let mir = scoop_mir_lower::lower(&output.local).expect("the concrete extension lowers to MIR");
    let source = mir
        .meta
        .source_callable_materializations
        .get_by_materialization(materialization)
        .expect("the extension has one source callable materialization");
    let signature = source.signature_record().signature();
    assert_eq!(
        signature.receiver(),
        scoop_identity::OptionalExactOwner::Present(exact_receiver)
    );
    assert_eq!(signature.parameters(), &[exact_parameter]);
    assert_eq!(
        signature.result(),
        output.local.exact_type_identities[function.return_ty].id()
    );
}

#[test]
fn explicit_receiver_extensions_fall_through_exact_current_star_then_core() {
    for winner in 0..4 {
        let parameter = |layer| if layer < winner { "String" } else { "Boolean" };
        let exact_source = package(
            file(vec![extension("Owner", "choose", parameter(0), 1)]),
            "exactlib",
        );
        let star_source = package(
            file(vec![extension("Owner", "choose", parameter(2), 3)]),
            "starlib",
        );
        let core = core_with(vec![
            struct_decl("Owner", Vec::new()),
            extension("Owner", "choose", parameter(3), 4),
        ]);
        let mut user = consumer(explicit_call("choose"));
        user.declarations
            .push(extension("Owner", "choose", parameter(1), 2));
        user.imports = vec![exact("exactlib", "choose", None), star("starlib")];

        for reverse in [false, true] {
            let mut sources = vec![exact_source.clone(), star_source.clone(), user.clone()];
            if reverse {
                sources.reverse();
                let user = sources
                    .iter_mut()
                    .find(|source| {
                        source
                            .declarations
                            .iter()
                            .any(|declaration| matches!(declaration, Decl::Function(function) if function.name.text == "main"))
                    })
                    .expect("one source contains main");
                user.imports.reverse();
            }
            let output = lower_sources(sources, core.clone())
                .expect("an inapplicable extension layer falls through");
            assert_marker(&output.export, chosen(&output), i64::from(winner + 1));
        }
    }
}

#[test]
fn ambiguous_exact_extension_layer_is_terminal() {
    let left = package(
        file(vec![extension("Owner", "choose", "Boolean", 1)]),
        "left",
    );
    let right = package(
        file(vec![extension("Owner", "choose", "Boolean", 2)]),
        "right",
    );
    let core = core_with(vec![
        struct_decl("Owner", Vec::new()),
        extension("Owner", "choose", "Boolean", 4),
    ]);
    let mut user = consumer(marked_explicit_call("choose"));
    user.declarations
        .push(extension("Owner", "choose", "Boolean", 3));
    user.imports = vec![
        exact("left", "choose", None),
        exact("right", "choose", None),
    ];

    for reverse in [false, true] {
        let mut user = user.clone();
        if reverse {
            user.imports.reverse();
        }
        let mut sources = vec![left.clone(), right.clone(), user];
        if reverse {
            sources.reverse();
        }
        let errors = lower_sources(sources, core.clone())
            .expect_err("an applicable ambiguous layer cannot fall through");
        assert!(
            errors.iter().any(|diagnostic| {
                diagnostic.span == Some(ast::Span::new(700, 710))
                    && diagnostic
                        .message
                        .contains("ambiguous in extension candidate layer")
            }),
            "{errors:?}"
        );
    }
}

#[test]
fn rejected_exact_ordinary_function_does_not_hide_a_star_extension_role() {
    let duplicate = |parameter| {
        fun_expr(
            "route",
            Vec::new(),
            vec![(parameter, ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(1),
        )
    };
    let exact_source = package(
        file(vec![duplicate("left"), duplicate("right")]),
        "exactlib",
    );
    let star_source = package(
        file(vec![extension("Owner", "route", "Boolean", 7)]),
        "starlib",
    );
    let mut user = consumer(method_call(
        call("Owner", Vec::new()),
        "route",
        vec![call("missingExtensionArgument", Vec::new())],
    ));
    user.imports = vec![exact("exactlib", "route", None), star("starlib")];

    let errors = lower_sources(
        vec![exact_source, star_source, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("the exact declarations are duplicate");
    assert!(errors.iter().any(|error| {
        error.message == "function `route` is already declared with the same signature"
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("missingExtensionArgument")),
        "the lower extension role must still probe its argument: {errors:?}"
    );
}

#[test]
fn rejected_exact_extension_blocks_a_star_extension_of_the_same_role() {
    let duplicate = |parameter| {
        extension_expr(
            ty_named("Owner"),
            "route",
            Vec::new(),
            vec![(parameter, ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(1),
        )
    };
    let exact_source = package(
        file(vec![duplicate("left"), duplicate("right")]),
        "exactlib",
    );
    let star_source = package(
        file(vec![extension("Owner", "route", "Boolean", 7)]),
        "starlib",
    );
    let mut user = consumer(method_call(
        call("Owner", Vec::new()),
        "route",
        vec![call("missingStarArgument", Vec::new())],
    ));
    user.imports = vec![exact("exactlib", "route", None), star("starlib")];

    let errors = lower_sources(
        vec![exact_source, star_source, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("the exact extension declarations are duplicate");
    assert_eq!(
        errors
            .iter()
            .filter(|error| {
                error.message == "function `route` is already declared with the same signature"
            })
            .count(),
        1,
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .all(|error| !error.message.contains("missingStarArgument")),
        "the lower extension layer must remain hidden: {errors:?}"
    );
}

#[test]
fn rejected_exact_function_does_not_hide_a_star_extension_property_role() {
    let duplicate = |parameter| {
        fun_expr(
            "route",
            Vec::new(),
            vec![(parameter, ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(1),
        )
    };
    let exact_source = package(
        file(vec![duplicate("left"), duplicate("right")]),
        "exactlib",
    );
    let star_source = package(
        file(vec![extension_property("Owner", "route", "Handler")]),
        "starlib",
    );
    let mut invoke = method_expr(
        "invoke",
        vec![("value", ty_named("Boolean"))],
        Some(ty_named("Int")),
        int_lit(7),
    );
    invoke.operator = Some(ast::OperatorModifier { span: sp() });
    let mut user = consumer(method_call(
        call("Owner", Vec::new()),
        "route",
        vec![call("missingPropertyArgument", Vec::new())],
    ));
    user.imports = vec![exact("exactlib", "route", None), star("starlib")];

    let errors = lower_sources(
        vec![exact_source, star_source, user],
        core_with(vec![
            struct_decl("Owner", Vec::new()),
            struct_decl_methods("Handler", Vec::new(), vec![invoke]),
        ]),
    )
    .expect_err("the exact declarations are duplicate");
    assert!(errors.iter().any(|error| {
        error.message == "function `route` is already declared with the same signature"
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("missingPropertyArgument")),
        "the lower extension-property role must still be callable: {errors:?}"
    );
}

#[test]
fn exact_alias_does_not_erase_an_extension_operator_role() {
    let library = package(
        file(vec![operator(extension("Owner", "plus", "Owner", 9))]),
        "operators",
    );
    let mut user = consumer(binary(
        BinOp::Add,
        call("Owner", Vec::new()),
        call("Owner", Vec::new()),
    ));
    user.imports = vec![exact("operators", "plus", Some("hiddenPlus"))];
    let output = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("operator lookup follows the imported callable's typed role");
    assert_marker(&output.export, chosen(&output), 9);
}

#[test]
fn member_property_with_extension_invoke_keeps_the_invoke_layer() {
    for exact_applicable in [false, true] {
        let core = core_with(vec![
            struct_decl("Handler", Vec::new()),
            owner_with_member_property("Owner", "Handler"),
        ]);
        let direct = package(
            file(vec![extension(
                "Owner",
                "route",
                if exact_applicable {
                    "Boolean"
                } else {
                    "String"
                },
                2,
            )]),
            "direct",
        );
        let invocation = package(file(vec![invoke("Handler", 7)]), "invocation");
        let mut user = consumer(explicit_call("route"));
        user.imports = vec![exact("direct", "route", None), star("invocation")];

        let output = lower_sources(vec![direct, invocation, user], core)
            .expect("the member property path remains callable in the invoke layer");
        assert_marker(
            &output.export,
            chosen(&output),
            if exact_applicable { 2 } else { 7 },
        );
    }
}

#[test]
fn extension_property_invoke_rank_is_the_worse_of_both_origins() {
    for property_is_exact in [false, true] {
        for current_applicable in [false, true] {
            let core = core_with(vec![
                struct_decl("Owner", Vec::new()),
                struct_decl("Handler", Vec::new()),
            ]);
            let exact_source = if property_is_exact {
                package(
                    file(vec![extension_property("Owner", "route", "Handler")]),
                    "exactlib",
                )
            } else {
                package(file(vec![invoke("Handler", 7)]), "exactlib")
            };
            let star_source = if property_is_exact {
                package(file(vec![invoke("Handler", 7)]), "starlib")
            } else {
                package(
                    file(vec![extension_property("Owner", "route", "Handler")]),
                    "starlib",
                )
            };
            let mut user = consumer(explicit_call("route"));
            user.declarations.push(extension(
                "Owner",
                "route",
                if current_applicable {
                    "Boolean"
                } else {
                    "String"
                },
                2,
            ));
            user.imports = if property_is_exact {
                vec![exact("exactlib", "route", None), star("starlib")]
            } else {
                vec![exact("exactlib", "invoke", None), star("starlib")]
            };

            let output = lower_sources(vec![exact_source, star_source, user], core)
                .expect("property-like invoke uses max(property rank, invoke rank)");
            assert_marker(
                &output.export,
                chosen(&output),
                if current_applicable { 2 } else { 7 },
            );
        }
    }
}

#[test]
fn crossed_property_invoke_pairs_share_one_effective_msc_partition() {
    let core = core_with(vec![
        struct_decl("Owner", Vec::new()),
        struct_decl("FirstHandler", Vec::new()),
        struct_decl("SecondHandler", Vec::new()),
    ]);
    let high_declarations = vec![
        extension_property("Owner", "route", "FirstHandler"),
        invoke("SecondHandler", 8),
    ];
    let low_declarations = vec![
        extension_property("Owner", "route", "SecondHandler"),
        invoke("FirstHandler", 7),
    ];

    for reverse in [false, true] {
        let ordered = |mut declarations: Vec<Decl>, name| {
            if reverse {
                declarations.reverse();
            }
            package(file(declarations), name)
        };
        let high = ordered(high_declarations.clone(), "high");
        let low = ordered(low_declarations.clone(), "low");
        let mut user = consumer(marked_explicit_call("route"));
        user.imports = vec![
            exact("high", "route", None),
            exact("high", "invoke", None),
            star("low"),
        ];
        if reverse {
            user.imports.reverse();
        }
        let mut sources = vec![high, low, user];
        if reverse {
            sources.reverse();
        }

        let errors = lower_sources(sources, core.clone()).expect_err(
            "crossed exact/star property-invoke pairs tie in one effective star partition",
        );
        assert!(
            errors.iter().any(|diagnostic| {
                diagnostic.span == Some(ast::Span::new(700, 710))
                    && diagnostic.message.contains("call to `invoke` is ambiguous")
                    && diagnostic.message.contains("star import layer")
            }),
            "{errors:?}"
        );
    }
}
