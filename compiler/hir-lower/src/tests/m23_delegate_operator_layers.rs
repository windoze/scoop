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

fn exact(package: &str, alias: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(&[package, "getValue"]),
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

fn core_with(delegate_methods: Vec<FunctionDecl>, extensions: Vec<Decl>) -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.extend([
        struct_decl_methods("Delegate", Vec::new(), delegate_methods),
        struct_decl("OtherDelegate", Vec::new()),
    ]);
    core.declarations.extend(extensions);
    make_core_public(&mut core);
    core
}

fn role_extension(receiver: &str, marker: i64) -> Decl {
    let Decl::Function(mut function) = extension_expr(
        ty_named(receiver),
        "getValue",
        Vec::new(),
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("Int")),
        int_lit(marker),
    ) else {
        panic!("extension builder returns a function")
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    Decl::Function(function)
}

fn role_method(this_ref: &str, marker: i64) -> FunctionDecl {
    let mut function = method_expr(
        "getValue",
        vec![("thisRef", ty_named(this_ref))],
        Some(ty_named("Int")),
        int_lit(marker),
    );
    function.operator = Some(ast::OperatorModifier { span: sp() });
    function
}

fn local_delegate(name: &str, expression: Expr) -> Statement {
    Statement {
        kind: StatementKind::LocalDelegatedProperty(ast::LocalDelegatedPropertyDecl {
            mutable: false,
            name: ident(name),
            ty: Some(ty_named("Int")),
            expression,
            by_span: sp(),
            span: sp(),
        }),
        span: sp(),
    }
}

fn consumer() -> ast::SourceFile {
    file(vec![fun(
        "main",
        vec![
            local_delegate("number", call("Delegate", Vec::new())),
            val("observed", var("number")),
        ],
    )])
}

fn body<'a>(module: &'a hir::Module, name: &str) -> &'a hir::Body {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("test function `{name}` exists"));
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function `{name}` has a source body")
    };
    body
}

fn selected_getter(module: &hir::Module) -> hir::FunctionId {
    let expression = local_init(body(module, "main"), "observed");
    match expression.kind {
        hir::ExprKind::Call { callee, .. } => module.callable_function(callee),
        hir::ExprKind::MethodCall { callee, .. } => module.callable_function(callee),
        ref other => panic!("delegate read is a resolved role call, found {other:?}"),
    }
}

fn selected_marker(module: &hir::Module) -> i64 {
    let function = &module.functions[selected_getter(module)];
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("selected getter has a source body")
    };
    let hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(marker)) =
        return_value(&body.statements).kind
    else {
        panic!("selected getter returns its integer marker")
    };
    i64::from(marker)
}

#[test]
fn delegate_extension_role_falls_through_exact_current_star_then_core() {
    for winner in 0..4 {
        let receiver = |layer| {
            if layer < winner {
                "OtherDelegate"
            } else {
                "Delegate"
            }
        };
        let exact_source = package(file(vec![role_extension(receiver(0), 1)]), "exactlib");
        let star_source = package(file(vec![role_extension(receiver(2), 3)]), "starlib");
        let core = core_with(Vec::new(), vec![role_extension(receiver(3), 4)]);
        let mut user = consumer();
        user.declarations.push(role_extension(receiver(1), 2));
        user.imports = vec![exact("exactlib", "renamedGet"), star("starlib")];

        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("delegate roles use fixed layers and skip inapplicable candidates");
        assert_eq!(selected_marker(&output.export), i64::from(winner + 1));
    }
}

#[test]
fn duplicate_star_imports_reach_one_typed_delegate_role_origin() {
    let library = package(file(vec![role_extension("Delegate", 3)]), "api");
    let mut user = consumer();
    user.imports = vec![star("api"), star("api")];

    let output = lower_sources(vec![library, user], core_with(Vec::new(), Vec::new()))
        .expect("duplicate import paths do not duplicate a delegate role candidate");
    assert_eq!(selected_marker(&output.export), 3);
}

#[test]
fn inaccessible_current_role_does_not_hide_visible_star_role() {
    let mut hidden = role_extension("Delegate", 2);
    let Decl::Function(function) = &mut hidden else {
        panic!("role extension builder returns a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let library = package(file(vec![role_extension("Delegate", 3)]), "api");
    let mut user = consumer();
    user.imports.push(star("api"));

    let output = lower_sources(
        vec![file(vec![hidden]), library, user],
        core_with(Vec::new(), Vec::new()),
    )
    .expect("delegate role visibility is checked before applicability");
    assert_eq!(selected_marker(&output.export), 3);
}

#[test]
fn applicable_exact_delegate_role_ambiguity_is_terminal() {
    let left = package(file(vec![role_extension("Delegate", 1)]), "left");
    let right = package(file(vec![role_extension("Delegate", 2)]), "right");
    let mut user = consumer();
    user.declarations.push(role_extension("Delegate", 3));
    user.imports = vec![exact("left", "leftGet"), exact("right", "rightGet")];

    let errors = lower_sources(vec![left, right, user], core_with(Vec::new(), Vec::new()))
        .expect_err("an ambiguous exact role layer cannot fall through to current package");
    assert!(
        errors.iter().any(|error| {
            error.message.contains("getValue") && error.message.contains("ambiguous")
        }),
        "{errors:?}"
    );
}

#[test]
fn applicable_member_role_precedes_extension() {
    let library = package(file(vec![role_extension("Delegate", 2)]), "api");
    let mut user = consumer();
    user.imports.push(exact("api", "aliasedGet"));

    let output = lower_sources(
        vec![library, user],
        core_with(vec![role_method("Unit", 1)], Vec::new()),
    )
    .expect("a real member delegate role precedes every extension layer");
    assert_eq!(selected_marker(&output.export), 1);
}

#[test]
fn inapplicable_member_role_falls_through_to_exact_extension() {
    let library = package(file(vec![role_extension("Delegate", 2)]), "api");
    let mut user = consumer();
    user.imports.push(exact("api", "aliasedGet"));

    let output = lower_sources(
        vec![library, user],
        core_with(vec![role_method("String", 1)], Vec::new()),
    )
    .expect("an inapplicable member role does not hide an exact extension role");
    assert_eq!(selected_marker(&output.export), 2);
}
