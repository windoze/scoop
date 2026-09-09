use super::*;

#[test]
fn mixed_inapplicable_partition_reports_each_typed_origin() {
    let mut user = consumer(call("Choose", vec![str_lit("bad")]));
    user.declarations.extend([
        struct_decl("Choose", vec![("value", ty_named("Int"))]),
        tagged("Choose", "Boolean", 1),
    ]);
    let errors = lower_sources(vec![user], core_file()).expect_err("both kinds are inapplicable");
    assert!(
        errors.iter().any(
            |diagnostic| diagnostic.message.contains("no applicable candidate")
                && diagnostic.message.contains("Choose(value: Boolean)")
                && diagnostic.message.contains("value: Int")
                && diagnostic.message.contains("String")
        ),
        "{errors:?}"
    );
}

#[test]
fn object_without_invoke_keeps_its_non_constructor_diagnostic() {
    let mut user = consumer(call("Only", Vec::new()));
    user.declarations.push(Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident("Only"),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    }));
    let errors = lower_sources(vec![user], core_file()).expect_err("objects have no constructor");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message == "object `Only` cannot be constructed"),
        "{errors:?}"
    );
}

fn path(parts: &[&str]) -> ast::QualifiedNameSyntax {
    ast::QualifiedNameSyntax {
        first: ident(parts[0]),
        rest: parts[1..]
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
        path: path(&[name]),
        span: sp(),
    };
    source
}

fn exact(parts: &[&str], alias: Option<&str>) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: path(parts),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn star(name: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Star {
        exposure: ast::ImportExposureSyntax::Local,
        namespace: path(&[name]),
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
    let input = LegacyCombinedSources::try_new(
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
    lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn tagged(name: &str, parameter: &str, marker: i64) -> Decl {
    fun_expr(
        name,
        Vec::new(),
        vec![("value", ty_named(parameter))],
        Some(ty_named("Int")),
        int_lit(marker),
    )
}

fn consumer(expression: Expr) -> ast::SourceFile {
    file(vec![fun("main", vec![val("chosen", expression)])])
}

fn public_core(mut declaration: Decl) -> Decl {
    let visibility = || ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    match &mut declaration {
        Decl::Function(function) => function.visibility = visibility(),
        Decl::Struct(structure) => {
            structure.visibility = visibility();
            for member in &mut structure.members {
                if let ast::StructMember::Function(function) = member {
                    function.visibility = visibility();
                }
            }
        }
        _ => panic!("test core exports are functions or structures"),
    }
    declaration
}

fn marked_call(name: &str) -> Expr {
    let mut expression = call_at(name, vec![bool_lit(true)], ast::Span::new(100, 106));
    let Expr::Call(call) = &mut expression else {
        unreachable!()
    };
    call.span = ast::Span::new(100, 106);
    call.args[0].span = ast::Span::new(100, 106);
    call.args[0].expression = Expr::BoolLiteral {
        value: true,
        span: ast::Span::new(100, 106),
    };
    expression
}

fn body<'a>(module: &'a hir::Module, name: &str) -> &'a hir::Body {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .expect("test function exists");
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function has a body")
    };
    body
}

fn chosen(output: &hir::Output) -> &hir::Expr {
    local_init(body(&output.export, "main"), "chosen")
}

fn local_origin<'a>(body: &'a hir::Body, mut expression: &'a hir::Expr) -> &'a hir::Expr {
    let mut visited = Vec::new();
    while let hir::ExprKind::Local(local) = expression.kind {
        assert!(
            !visited.contains(&local),
            "temporary initializers must be acyclic"
        );
        visited.push(local);
        expression = body
            .statements
            .iter()
            .find_map(|statement| {
                let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                    return None;
                };
                (binding_local(pattern) == Some(local)).then_some(init)
            })
            .expect("materialized receiver has a typed local initializer");
    }
    expression
}

fn assert_marker(module: &hir::Module, expression: &hir::Expr, marker: u32) {
    let callable = match &expression.kind {
        hir::ExprKind::Call { callee, .. } => *callee,
        hir::ExprKind::MethodCall {
            callee: hir::MethodCallee::Callable(callee),
            ..
        } => *callee,
        other => panic!("expected a typed callable, got {other:?}"),
    };
    let function = &module.functions[module.callable_function(callable)];
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("selected function is user-defined")
    };
    assert!(
        matches!(return_value(&body.statements).kind,
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(value)) if value == marker),
        "selected the wrong declaration: {}",
        function.name
    );
}

fn extension(receiver: TypeRef, name: &str, marker: i64) -> Decl {
    extension_expr(
        receiver,
        name,
        Vec::new(),
        vec![("value", ty_named("Boolean"))],
        Some(ty_named("Int")),
        int_lit(marker),
    )
}

fn operator(mut declaration: Decl) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("operator is a function")
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    declaration
}

fn handler(member_invoke: bool) -> Decl {
    let methods = if member_invoke {
        let mut invoke = method_expr(
            "invoke",
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Int")),
            int_lit(7),
        );
        invoke.operator = Some(ast::OperatorModifier { span: sp() });
        vec![invoke]
    } else {
        Vec::new()
    };
    struct_decl_methods("Handler", Vec::new(), methods)
}

fn property(name: &str, ty: TypeRef) -> Decl {
    property_value(name, ty, call("Handler", Vec::new()))
}

fn property_value(name: &str, ty: TypeRef, expression: Expr) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(expression)),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    })
}

#[test]
fn first_applicable_named_layer_uses_exact_current_star_then_core() {
    for winner in 0..4 {
        let parameter = |layer| if layer < winner { "Int" } else { "Boolean" };
        let exact_source = package(file(vec![tagged("choose", parameter(0), 1)]), "exactlib");
        let star_source = package(file(vec![tagged("choose", parameter(2), 3)]), "starlib");
        let mut core = core_file();
        core.declarations
            .push(public_core(tagged("choose", "Boolean", 4)));
        let mut user = consumer(call("choose", vec![bool_lit(true)]));
        user.declarations.push(tagged("choose", parameter(1), 2));
        user.imports = vec![exact(&["exactlib", "choose"], None), star("starlib")];
        for reverse in [false, true] {
            let mut sources = vec![exact_source.clone(), star_source.clone(), user.clone()];
            if reverse {
                sources.reverse();
                sources[0].imports.reverse();
            }
            let output = lower_sources(sources, core.clone())
                .expect("higher non-applicable types fall through");
            assert_marker(&output.export, chosen(&output), winner + 1);
        }
    }
}

#[test]
fn incompatible_named_argument_shape_falls_through_exact_import() {
    let mut imported = tagged("choose", "Boolean", 1);
    let Decl::Function(function) = &mut imported else {
        unreachable!()
    };
    function.params[0].name = ident("other");
    let mut user = consumer(source_call(
        "choose",
        vec![named_argument("value", bool_lit(true))],
    ));
    user.declarations.push(tagged("choose", "Boolean", 2));
    user.imports.push(exact(&["api", "choose"], None));
    let output = lower_sources(
        vec![package(file(vec![imported]), "api"), user],
        core_file(),
    )
    .expect("shape failure must not shadow current package");
    assert_marker(&output.export, chosen(&output), 2);
}

#[test]
fn failed_named_call_reports_the_source_use_after_exhausting_layers() {
    let mut user = consumer(marked_call("choose"));
    user.declarations.push(tagged("choose", "Int", 2));
    user.imports.push(exact(&["api", "choose"], None));
    let errors = lower_sources(
        vec![
            package(file(vec![tagged("choose", "String", 1)]), "api"),
            user,
        ],
        core_file(),
    )
    .expect_err("neither callable is applicable");
    assert!(
        errors
            .iter()
            .any(|error| error.span == Some(ast::Span::new(100, 106))
                && error.message.contains("choose")),
        "{errors:?}"
    );
}

#[test]
fn concrete_function_beats_generic_constructor_in_one_partition() {
    let mut user = consumer(call("Choose", vec![bool_lit(true)]));
    user.declarations.extend([
        generic_struct_decl("Choose", vec!["T"], vec![("value", ty_named("T"))]),
        tagged("Choose", "Boolean", 5),
    ]);
    let output =
        lower_sources(vec![user], core_file()).expect("function participates in constructor MSC");
    assert_marker(&output.export, chosen(&output), 5);
}

#[test]
fn concrete_constructor_beats_generic_function_in_one_partition() {
    let mut user = consumer(call("Choose", vec![bool_lit(true)]));
    user.declarations.extend([
        struct_decl("Choose", vec![("value", ty_named("Boolean"))]),
        fun_expr(
            "Choose",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("Int")),
            int_lit(5),
        ),
    ]);
    let output =
        lower_sources(vec![user], core_file()).expect("constructor participates in function MSC");
    assert!(matches!(
        output.export.types[chosen(&output).ty],
        hir::Type::Struct(_)
    ));
}

#[test]
fn star_imported_nominal_constructor_is_a_named_call_candidate() {
    let library = package(
        file(vec![struct_decl(
            "ImportedBox",
            vec![("value", ty_named("Boolean"))],
        )]),
        "api",
    );
    let mut user = consumer(call("ImportedBox", vec![bool_lit(true)]));
    user.imports.push(star("api"));

    let output = lower_sources(vec![library, user], core_file())
        .expect("a nominal imported through a star is callable as its constructor");
    assert!(matches!(
        output.export.types[chosen(&output).ty],
        hir::Type::Struct(_)
    ));
}

#[test]
fn fixed_typealias_application_is_not_a_generic_constructor_candidate() {
    let library = package(
        file(vec![
            generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
            Decl::TypeAlias(ast::TypeAliasDecl {
                visibility: ast::VisibilitySyntax::Omitted,
                name: ident("Fixed"),
                target: ty_generic("Box", vec![ty_named("Boolean")]),
                span: sp(),
            }),
            fun_expr(
                "make",
                vec!["T"],
                vec![("value", ty_named("T"))],
                Some(ty_named("Int")),
                int_lit(5),
            ),
        ]),
        "api",
    );
    let mut user = consumer(call("Choose", vec![bool_lit(true)]));
    user.imports = vec![
        exact(&["api", "Fixed"], Some("Choose")),
        exact(&["api", "make"], Some("Choose")),
    ];
    let output = lower_sources(vec![library, user], core_file())
        .expect("fixed alias is more specific than generic callable");
    assert!(matches!(
        output.export.types[chosen(&output).ty],
        hir::Type::Struct(_)
    ));
}

#[test]
fn equally_specific_function_and_constructor_are_ambiguous_in_any_import_order() {
    let left = package(
        file(vec![struct_decl(
            "Item",
            vec![("value", ty_named("Boolean"))],
        )]),
        "left",
    );
    let right = package(file(vec![tagged("make", "Boolean", 5)]), "right");
    let mut user = consumer(marked_call("Choose"));
    user.imports = vec![
        exact(&["left", "Item"], Some("Choose")),
        exact(&["right", "make"], Some("Choose")),
    ];
    for reverse in [false, true] {
        if reverse {
            user.imports.reverse();
        }
        let errors = lower_sources(vec![left.clone(), right.clone(), user.clone()], core_file())
            .expect_err("cross-kind ties cannot use declaration or import order");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("ambiguous")
                    && error.span == Some(ast::Span::new(100, 106))),
            "{errors:?}"
        );
    }
}

#[test]
fn exact_property_with_member_invoke_precedes_current_function() {
    let library = package(
        file(vec![handler(true), property("choose", ty_named("Handler"))]),
        "api",
    );
    let mut user = consumer(call("choose", vec![bool_lit(true)]));
    user.declarations.push(tagged("choose", "Boolean", 2));
    user.imports.push(exact(&["api", "choose"], None));
    let output = lower_sources(vec![library, user], core_file())
        .expect("member invoke retains exact property layer");
    assert_marker(&output.export, chosen(&output), 7);
}

#[test]
fn exact_property_with_lower_extension_invoke_uses_the_lower_layer() {
    for core_invoke in [false, true] {
        for current_function in [false, true] {
            let mut core = core_file();
            core.declarations.push(public_core(handler(false)));
            let invoke = operator(extension(ty_named("Handler"), "invoke", 7));
            let mut library = package(file(vec![property("choose", ty_named("Handler"))]), "api");
            if core_invoke {
                core.declarations.push(public_core(invoke));
            } else {
                library.declarations.push(invoke);
            }
            let mut user = consumer(call("choose", vec![bool_lit(true)]));
            user.imports = vec![exact(&["api", "choose"], None), star("api")];
            if current_function {
                user.declarations.push(tagged("choose", "Boolean", 2));
            }
            let output = lower_sources(vec![library, user], core)
                .expect("property and invoke take their lower effective layer");
            assert_marker(
                &output.export,
                chosen(&output),
                if current_function { 2 } else { 7 },
            );
        }
    }
}

#[test]
fn star_property_with_exact_extension_invoke_stays_below_current_function() {
    let mut core = core_file();
    core.declarations.push(public_core(handler(false)));
    let library = package(
        file(vec![
            property("choose", ty_named("Handler")),
            operator(extension(ty_named("Handler"), "invoke", 7)),
        ]),
        "api",
    );
    let mut user = consumer(call("choose", vec![bool_lit(true)]));
    user.imports = vec![star("api"), exact(&["api", "invoke"], None)];
    user.declarations.push(tagged("choose", "Boolean", 2));
    let output = lower_sources(vec![library, user], core)
        .expect("exact invoke cannot promote a star property");
    assert_marker(&output.export, chosen(&output), 2);
}

#[test]
fn implicit_this_real_member_precedes_exact_import() {
    let library = package(file(vec![tagged("choose", "Boolean", 1)]), "api");
    let mut user = file(vec![
        struct_decl_methods(
            "Host",
            Vec::new(),
            vec![
                method_expr(
                    "choose",
                    vec![("value", ty_named("Boolean"))],
                    Some(ty_named("Int")),
                    int_lit(8),
                ),
                method_expr(
                    "run",
                    Vec::new(),
                    Some(ty_named("Int")),
                    call("choose", vec![bool_lit(true)]),
                ),
            ],
        ),
        fun("main", Vec::new()),
    ]);
    user.imports.push(exact(&["api", "choose"], None));
    let output =
        lower_sources(vec![library, user], core_file()).expect("implicit member outranks imports");
    assert_marker(
        &output.export,
        return_value(&body(&output.export, "Host.run").statements),
        8,
    );
}

#[test]
fn initializing_receiver_callable_property_remains_a_real_member() {
    let thunk = ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident("thunk"),
        ty: ty_function(false, Vec::new(), ty_named("Int")),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(Expr::Lambda {
                id: ast::LambdaId(0),
                is_suspend: false,
                parameters: None,
                body: block(vec![stmt(int_lit(7))]),
                span: sp(),
            }),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    };
    let mut host_decl = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(host) = &mut host_decl else {
        panic!("class builder creates a class")
    };
    host.members.extend([
        ast::ClassMember::StoredProperty(thunk),
        ast::ClassMember::InitBlock(ast::InitBlockDecl {
            body: block(vec![stmt(call("thunk", Vec::new()))]),
            span: sp(),
        }),
    ]);

    lower_sources(
        vec![file(vec![host_decl, fun("main", Vec::new())])],
        core_file(),
    )
    .expect("an initialized callable-valued property is invocable during initialization");
}

#[test]
fn implicit_this_extension_participates_in_its_import_layer() {
    let mut core = core_file();
    core.declarations
        .push(public_core(struct_decl("Receiver", Vec::new())));
    let library = package(
        file(vec![extension(ty_named("Receiver"), "choose", 1)]),
        "api",
    );
    let mut user = file(vec![
        extension_expr(
            ty_named("Receiver"),
            "run",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            call("choose", vec![bool_lit(true)]),
        ),
        tagged("choose", "Boolean", 2),
        fun("main", Vec::new()),
    ]);
    user.imports.push(exact(&["api", "choose"], None));
    let output = lower_sources(vec![library, user], core)
        .expect("implicit extension is not delayed behind all top-level functions");
    assert_marker(
        &output.export,
        return_value(&body(&output.export, "run").statements),
        1,
    );
}

#[test]
fn explicit_receiver_uses_member_then_exact_current_star_core_extensions() {
    for winner in 0..5 {
        let mut core = core_file();
        let methods = if winner == 0 {
            vec![method_expr(
                "choose",
                vec![("value", ty_named("Boolean"))],
                Some(ty_named("Int")),
                int_lit(1),
            )]
        } else {
            Vec::new()
        };
        core.declarations.extend([
            public_core(struct_decl_methods("Receiver", Vec::new(), methods)),
            public_core(extension(ty_named("Receiver"), "choose", 5)),
        ]);
        let candidate = |layer: u32, marker| {
            let receiver = if layer < winner {
                ty_named("String")
            } else {
                ty_named("Receiver")
            };
            extension(receiver, "choose", marker)
        };
        let exact_source = package(file(vec![candidate(1, 2)]), "exactlib");
        let star_source = package(file(vec![candidate(3, 4)]), "starlib");
        let mut user = consumer(method_call(
            call("Receiver", Vec::new()),
            "choose",
            vec![bool_lit(true)],
        ));
        user.declarations.push(candidate(2, 3));
        user.imports = vec![exact(&["exactlib", "choose"], None), star("starlib")];
        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("extension receiver incompatibility falls through");
        assert_marker(&output.export, chosen(&output), winner + 1);
    }
}

#[test]
fn inaccessible_current_candidate_does_not_hide_deduplicated_star_function() {
    let mut private = tagged("choose", "Boolean", 2);
    let Decl::Function(function) = &mut private else {
        unreachable!()
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let hidden = file(vec![private]);
    let library = package(file(vec![tagged("choose", "Boolean", 3)]), "api");
    let mut user = consumer(call("choose", vec![bool_lit(true)]));
    user.imports = vec![star("api"), star("api")];
    for reverse in [false, true] {
        let mut sources = vec![hidden.clone(), library.clone(), user.clone()];
        if reverse {
            sources.reverse();
        }
        let output = lower_sources(sources, core_file())
            .expect("visibility filters before applicability and repeated origins deduplicate");
        assert_marker(&output.export, chosen(&output), 3);
    }
}

#[test]
fn applicable_exact_ambiguity_does_not_fall_through_to_current_package() {
    let left = package(file(vec![tagged("choose", "Boolean", 1)]), "left");
    let right = package(file(vec![tagged("choose", "Boolean", 2)]), "right");
    let mut user = consumer(marked_call("choose"));
    user.declarations.push(tagged("choose", "Boolean", 3));
    user.imports = vec![
        exact(&["left", "choose"], None),
        exact(&["right", "choose"], None),
    ];
    for reverse in [false, true] {
        if reverse {
            user.imports.reverse();
        }
        let errors = lower_sources(vec![left.clone(), right.clone(), user.clone()], core_file())
            .expect_err("an applicable ambiguous layer is terminal, not a fallback trigger");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("ambiguous")
                    && error.span == Some(ast::Span::new(100, 106))),
            "{errors:?}"
        );
    }
}

#[test]
fn imported_singleton_method_and_property_invoke_preserve_typed_receivers() {
    let Decl::Global(handler_property) = property("handler", ty_named("Handler")) else {
        unreachable!()
    };
    let library = package(
        file(vec![
            handler(true),
            Decl::Object(ast::ObjectDecl {
                annotations: Vec::new(),
                visibility: ast::VisibilitySyntax::Omitted,
                name: ident("Tools"),
                supertypes: Vec::new(),
                members: vec![
                    ast::ClassMember::Function(method_expr(
                        "choose",
                        vec![("value", ty_named("Boolean"))],
                        Some(ty_named("Int")),
                        int_lit(9),
                    )),
                    ast::ClassMember::StoredProperty(handler_property),
                ],
                span: sp(),
            }),
        ]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![
            val("chosen", call("choose", vec![bool_lit(true)])),
            val("invoked", call("handler", vec![bool_lit(true)])),
        ],
    )]);
    user.imports = vec![
        exact(&["api", "Tools", "choose"], None),
        exact(&["api", "Tools", "handler"], None),
    ];
    let output = lower_sources(vec![library, user], core_file())
        .expect("imported object members retain their singleton receiver");
    assert_marker(&output.export, chosen(&output), 9);
    let hir::ExprKind::MethodCall { receiver, .. } = &chosen(&output).kind else {
        panic!("an imported singleton method remains a method call")
    };
    let main_body = body(&output.export, "main");
    let hir::ExprKind::SingletonValue(singleton) = local_origin(main_body, receiver).kind else {
        panic!("the imported method carries the singleton value")
    };
    let invoked = local_init(main_body, "invoked");
    assert_marker(&output.export, invoked, 7);
    let hir::ExprKind::MethodCall {
        receiver: property_read,
        ..
    } = &invoked.kind
    else {
        panic!("the property result is the invoke receiver")
    };
    let hir::ExprKind::MethodCall {
        receiver: property_owner,
        ..
    } = &local_origin(main_body, property_read).kind
    else {
        panic!("the property receiver is produced by its typed getter")
    };
    assert!(matches!(local_origin(main_body, property_owner).kind,
        hir::ExprKind::SingletonValue(value) if value == singleton));
}

#[test]
fn repeated_property_origin_with_star_invoke_is_one_effective_candidate() {
    let mut core = core_file();
    core.declarations.push(public_core(handler(false)));
    let library = package(
        file(vec![
            property("choose", ty_named("Handler")),
            operator(extension(ty_named("Handler"), "invoke", 7)),
        ]),
        "api",
    );
    let mut user = consumer(call("choose", vec![bool_lit(true)]));
    user.imports = vec![exact(&["api", "choose"], None), star("api"), star("api")];
    for reverse in [false, true] {
        if reverse {
            user.imports.reverse();
        }
        let mut sources = vec![library.clone(), user.clone()];
        if reverse {
            sources.reverse();
        }
        let output = lower_sources(sources, core.clone())
            .expect("exact and star paths to one property/invoke pair deduplicate");
        assert_marker(&output.export, chosen(&output), 7);
    }
}

#[test]
fn distinct_property_invoke_pairs_at_one_effective_layer_are_not_first_match() {
    let mut core = core_file();
    core.declarations.extend([
        public_core(handler(false)),
        public_core(struct_decl("OtherHandler", Vec::new())),
    ]);
    let high = package(
        file(vec![
            property("choose", ty_named("Handler")),
            operator(extension(ty_named("OtherHandler"), "invoke", 8)),
        ]),
        "high",
    );
    let low = package(
        file(vec![
            property_value(
                "choose",
                ty_named("OtherHandler"),
                call("OtherHandler", Vec::new()),
            ),
            operator(extension(ty_named("Handler"), "invoke", 7)),
        ]),
        "low",
    );
    let mut user = consumer(marked_call("choose"));
    user.imports = vec![
        exact(&["high", "choose"], None),
        exact(&["high", "invoke"], None),
        star("low"),
    ];
    for reverse in [false, true] {
        if reverse {
            user.imports.reverse();
        }
        let mut sources = vec![high.clone(), low.clone(), user.clone()];
        if reverse {
            sources.reverse();
        }
        let errors = lower_sources(sources, core.clone()).expect_err(
            "exact property/star invoke and star property/exact invoke share one ambiguous layer",
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("ambiguous")
                    && error.span == Some(ast::Span::new(100, 106))),
            "{errors:?}"
        );
    }
}
