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

fn exact(package: &str, name: &str, alias: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(&[package, name]),
        alias: Some(ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(alias),
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
    let request = ast::Stage1RequestId::from_raw(902);
    let mut parsed = sources.into_iter().enumerate().map(|(index, source)| {
        ast::ParsedSource::new(
            ast::Stage1SourceHandle::new(
                request,
                u32::try_from(index).expect("test source index fits u32"),
            ),
            source,
        )
    });
    let parsed = ast::AllParsedSources::try_new(
        request,
        ast::NonEmptyVec::new(
            parsed.next().expect("test source list is nonempty"),
            parsed.collect(),
        ),
    )
    .expect("all test sources belong to one request");
    let input = Stage1CompilationInput::new(
        vec![ProviderSource {
            source: &core,
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| Stage1SourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    );
    lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn receiver_struct(name: &str, marker: i64) -> Decl {
    struct_decl_methods(
        name,
        Vec::new(),
        vec![method_expr(
            "mark",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(marker),
        )],
    )
}

fn receiver_object(name: &str, marker: i64) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: vec![ast::ClassMember::Function(method_expr(
            "mark",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(marker),
        ))],
        span: sp(),
    })
}

fn empty_object(name: &str) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn property(name: &str, ty: &str) -> Decl {
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
        ty: ty_named(ty),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(call(ty, Vec::new()))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    })
}

fn extension_property(receiver: &str, name: &str, ty: &str) -> Decl {
    let Decl::Global(mut property) = property(name, ty) else {
        panic!("property helper creates a global property")
    };
    property.receiver_ty = Some(ty_named(receiver));
    Decl::Global(property)
}

fn type_alias(name: &str, target: &str) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target: ty_named(target),
        span: sp(),
    })
}

fn consumer() -> ast::SourceFile {
    file(vec![fun(
        "main",
        vec![val(
            "chosen",
            method_call(var("Receiver"), "mark", Vec::new()),
        )],
    )])
}

fn lambda(tail: Expr) -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: block(vec![stmt(tail)]),
        span: sp(),
    }
}

fn selected_marker(output: &hir::Output) -> u32 {
    let main = output
        .export
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "main").then_some(function))
        .expect("main function exists");
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main has a source body")
    };
    let expression = local_init(body, "chosen");
    let hir::ExprKind::MethodCall {
        callee: hir::MethodCallee::Callable(callee),
        ..
    } = expression.kind
    else {
        panic!("selected expression is a method call: {expression:?}")
    };
    let function = &output.export.functions[output.export.callable_function(callee)];
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("selected method has a source body")
    };
    let value = return_value(&body.statements);
    let hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(marker)) = value.kind
    else {
        panic!("selected method returns its marker: {value:?}")
    };
    marker
}

fn core_receiver() -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.push(receiver_object("Receiver", 4));
    make_core_public(&mut core);
    core
}

#[test]
fn expression_receiver_uses_exact_current_star_then_core() {
    for winner in 0..4 {
        let exact_source = package(
            file(vec![
                receiver_struct("ExactReceiver", 1),
                property("exactReceiver", "ExactReceiver"),
            ]),
            "exactlib",
        );
        let star_source = package(
            file(vec![
                receiver_struct("StarReceiver", 3),
                property("Receiver", "StarReceiver"),
            ]),
            "starlib",
        );
        let mut user = consumer();
        if winner <= 1 {
            user.declarations.extend([
                receiver_struct("CurrentReceiver", 2),
                property("Receiver", "CurrentReceiver"),
            ]);
        }
        if winner == 0 {
            user.imports
                .push(exact("exactlib", "exactReceiver", "Receiver"));
        }
        if winner <= 2 {
            // Reaching the same property twice must not manufacture an
            // ambiguity: the source binding remains one typed origin.
            user.imports.extend([star("starlib"), star("starlib")]);
        }
        let output = lower_sources(vec![exact_source, star_source, user], core_receiver())
            .expect("the first expression-receiver layer selects deterministically");
        assert_eq!(selected_marker(&output), winner + 1);
    }
}

#[test]
fn exact_typealias_qualifier_precedes_a_current_value_receiver() {
    let api = package(
        file(vec![
            receiver_object("ImportedReceiver", 1),
            type_alias("ImportedAlias", "ImportedReceiver"),
        ]),
        "api",
    );
    let mut user = consumer();
    user.declarations.extend([
        receiver_struct("CurrentReceiver", 2),
        property("Receiver", "CurrentReceiver"),
    ]);
    user.imports.push(exact("api", "ImportedAlias", "Receiver"));

    let output = lower_sources(vec![api, user], core_file())
        .expect("an exact alias remains a typed type qualifier");
    assert_eq!(selected_marker(&output), 1);
}

#[test]
fn exact_value_receiver_shadows_a_lower_typealias_qualifier() {
    let api = package(
        file(vec![
            receiver_struct("ImportedReceiver", 1),
            property("importedReceiver", "ImportedReceiver"),
        ]),
        "api",
    );
    let mut user = consumer();
    user.declarations.extend([
        receiver_object("FallbackReceiver", 2),
        type_alias("Receiver", "FallbackReceiver"),
    ]);
    user.imports
        .push(exact("api", "importedReceiver", "Receiver"));

    let output = lower_sources(vec![api, user], core_file())
        .expect("a higher-layer value owns the expression receiver spelling");
    assert_eq!(selected_marker(&output), 1);
}

#[test]
fn failed_exact_value_member_lookup_does_not_fall_back_to_a_typealias() {
    let api = package(
        file(vec![
            struct_decl("EmptyReceiver", Vec::new()),
            property("importedReceiver", "EmptyReceiver"),
        ]),
        "api",
    );
    let mut user = consumer();
    user.declarations.extend([
        receiver_object("FallbackReceiver", 2),
        type_alias("Receiver", "FallbackReceiver"),
    ]);
    user.imports
        .push(exact("api", "importedReceiver", "Receiver"));

    let errors = lower_sources(vec![api, user], core_file())
        .expect_err("a selected value receiver cannot retry as a lower type qualifier");
    assert!(
        errors.iter().any(|error| {
            error.message.contains("EmptyReceiver") && error.message.contains("mark")
        }),
        "{errors:?}"
    );
}

#[test]
fn exact_function_blocks_a_lower_type_qualifier() {
    let api = package(file(vec![fun("importedReceiver", Vec::new())]), "api");
    let mut user = consumer();
    user.declarations.extend([
        receiver_object("FallbackReceiver", 2),
        type_alias("Receiver", "FallbackReceiver"),
    ]);
    user.imports
        .push(exact("api", "importedReceiver", "Receiver"));

    let errors = lower_sources(vec![api, user], core_file())
        .expect_err("a function is a terminal non-type binding in its source layer");
    assert!(
        errors.iter().any(|error| {
            error.message
                == "function `Receiver` is not a value; use `::Receiver` to create a callable reference"
        }),
        "{errors:?}"
    );
}

#[test]
fn applicable_exact_extension_property_blocks_a_lower_type_qualifier() {
    let mut api = package(
        file(vec![
            receiver_struct("ExtensionReceiver", 1),
            extension_property("Context", "importedReceiver", "ExtensionReceiver"),
        ]),
        "api",
    );
    api.imports.push(exact("app", "Context", "Context"));

    let context = class_decl(
        ast::ClassModifier::Final,
        "Context",
        Vec::new(),
        None,
        Vec::new(),
        vec![method_expr(
            "select",
            Vec::new(),
            Some(ty_named("Int")),
            method_call(var("Receiver"), "mark", Vec::new()),
        )],
    );
    let mut user = package(
        file(vec![
            empty_object("FallbackReceiver"),
            type_alias("Receiver", "FallbackReceiver"),
            context,
            fun(
                "main",
                vec![val(
                    "chosen",
                    method_call(call("Context", Vec::new()), "select", Vec::new()),
                )],
            ),
        ]),
        "app",
    );
    user.imports
        .push(exact("api", "importedReceiver", "Receiver"));

    lower_sources(vec![api, user], core_file())
        .expect("an applicable implicit extension property owns its exact-import spelling");
}

#[test]
fn inapplicable_exact_extension_property_falls_through_to_a_typealias() {
    let api = package(
        file(vec![
            struct_decl("OtherContext", Vec::new()),
            receiver_struct("UnusedReceiver", 1),
            extension_property("OtherContext", "importedReceiver", "UnusedReceiver"),
        ]),
        "api",
    );
    let context = class_decl(
        ast::ClassModifier::Final,
        "Context",
        Vec::new(),
        None,
        Vec::new(),
        vec![method_expr(
            "select",
            Vec::new(),
            Some(ty_named("Int")),
            method_call(var("Receiver"), "mark", Vec::new()),
        )],
    );
    let mut user = file(vec![
        receiver_object("FallbackReceiver", 2),
        type_alias("Receiver", "FallbackReceiver"),
        context,
        fun(
            "main",
            vec![val(
                "chosen",
                method_call(call("Context", Vec::new()), "select", Vec::new()),
            )],
        ),
    ]);
    user.imports
        .push(exact("api", "importedReceiver", "Receiver"));

    lower_sources(vec![api, user], core_file())
        .expect("NoApplicable extension property layers must not hide a lower typealias");
}

#[test]
fn local_function_blocks_a_top_level_type_qualifier() {
    let user = file(vec![
        receiver_object("FallbackReceiver", 2),
        type_alias("Receiver", "FallbackReceiver"),
        fun(
            "main",
            vec![
                local_fun_sig("Receiver", Vec::new(), Vec::new(), None, Vec::new()),
                val("chosen", method_call(var("Receiver"), "mark", Vec::new())),
            ],
        ),
    ]);

    let errors = lower_sources(vec![user], core_file())
        .expect_err("a lexical function cannot be bypassed as a type qualifier");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("Receiver") && {
                error.message.contains("not a value") || error.message.contains("unknown variable")
            }),
        "{errors:?}"
    );
}

#[test]
fn captured_value_blocks_a_top_level_type_qualifier() {
    let callback_ty = ty_function(false, Vec::new(), ty_named("Int"));
    let user = file(vec![
        receiver_struct("CapturedReceiver", 1),
        empty_object("FallbackReceiver"),
        type_alias("Receiver", "FallbackReceiver"),
        fun(
            "main",
            vec![
                val("Receiver", call("CapturedReceiver", Vec::new())),
                val_ty(
                    "callback",
                    Some(callback_ty),
                    lambda(method_call(var("Receiver"), "mark", Vec::new())),
                ),
                val("chosen", call("callback", Vec::new())),
            ],
        ),
    ]);

    lower_sources(vec![user], core_file())
        .expect("a captured receiver must win before a top-level typealias");
}

#[test]
fn constructor_parameter_blocks_a_top_level_type_qualifier() {
    let mut host = class_decl(
        ast::ClassModifier::Final,
        "Host",
        vec![(false, "Receiver", ty_named("ParameterReceiver"))],
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(host) = &mut host else {
        panic!("class builder creates a class")
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut host.constructor else {
        panic!("class builder creates a primary constructor")
    };
    constructor.parameters[0].property = ast::PrimaryParameterProperty::Plain;
    constructor.parameters[0].member_visibility = None;
    host.members
        .push(ast::ClassMember::InitBlock(ast::InitBlockDecl {
            body: block(vec![stmt(method_call(var("Receiver"), "mark", Vec::new()))]),
            span: sp(),
        }));
    let user = file(vec![
        receiver_struct("ParameterReceiver", 1),
        empty_object("FallbackReceiver"),
        type_alias("Receiver", "FallbackReceiver"),
        Decl::Class(host.clone()),
        fun(
            "main",
            vec![val(
                "host",
                call("Host", vec![call("ParameterReceiver", Vec::new())]),
            )],
        ),
    ]);

    lower_sources(vec![user], core_file())
        .expect("a constructor parameter must win before a top-level typealias");
}
