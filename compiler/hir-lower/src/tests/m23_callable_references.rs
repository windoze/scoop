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
    exact_path(&[package, name], alias)
}

fn exact_path(parts: &[&str], alias: Option<&str>) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(parts),
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

fn core_with(declarations: Vec<Decl>) -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.extend(declarations);
    make_core_public(&mut core);
    core
}

fn candidate(name: &str, parameter: &str, marker: i64) -> Decl {
    fun_expr(
        name,
        Vec::new(),
        vec![("value", ty_named(parameter))],
        Some(ty_named("Int")),
        int_lit(marker),
    )
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

fn object_with_method(object: &str, name: &str, marker: i64) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(object),
        supertypes: Vec::new(),
        members: vec![ast::ClassMember::Function(method_expr(
            name,
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(marker),
        ))],
        span: sp(),
    })
}

fn class_with_companion_method(host: &str, companion: &str, name: &str, marker: i64) -> Decl {
    let Decl::Class(mut declaration) = class_decl(
        ast::ClassModifier::Final,
        host,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        panic!("class builder returns a class")
    };
    declaration
        .members
        .push(ast::ClassMember::Companion(Box::new(
            ast::CompanionObjectDecl {
                annotations: Vec::new(),
                visibility: ast::VisibilitySyntax::Omitted,
                name: ast::CompanionNameSyntax::Named(ident(companion)),
                supertypes: Vec::new(),
                members: vec![ast::ClassMember::Function(method_expr(
                    name,
                    vec![("value", ty_named("Int"))],
                    Some(ty_named("Int")),
                    int_lit(marker),
                ))],
                span: sp(),
            },
        )));
    Decl::Class(declaration)
}

fn no_gc(mut declaration: Decl) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("NoGC test declaration is a function")
    };
    function.annotations.push(ast::Annotation {
        name: ident("NoGC"),
        args: Vec::new(),
        span: sp(),
    });
    declaration
}

fn reference(receiver: Option<Expr>, name: &str) -> Expr {
    Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: receiver.map(Box::new),
        name: ident(name),
        span: sp(),
    }
}

fn consumer(name: &str, receiver: Option<Expr>, parameters: Vec<TypeRef>) -> ast::SourceFile {
    file(vec![fun(
        "main",
        vec![val_ty(
            "selected",
            Some(ty_function(false, parameters, ty_named("Int"))),
            reference(receiver, name),
        )],
    )])
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

fn selected_function(module: &hir::Module) -> hir::FunctionId {
    let expression = local_init(body(module, "main"), "selected");
    let hir::ExprKind::CallableReference(reference) = expression.kind else {
        panic!("selected value is a callable reference")
    };
    match &module.callable_references[reference].target {
        hir::CallableReferenceTarget::Named(callee) => module.callable_function(*callee),
        hir::CallableReferenceTarget::BoundExtension { callee, .. } => {
            module.callable_function(*callee)
        }
        hir::CallableReferenceTarget::BoundMember { callee, .. } => {
            module.callable_function(*callee)
        }
        hir::CallableReferenceTarget::Local { .. } => {
            panic!("the selected declaration is not local")
        }
    }
}

fn selected_marker(module: &hir::Module) -> i64 {
    let function = &module.functions[selected_function(module)];
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("selected function has a source body")
    };
    let expression = return_value(&body.statements);
    let hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(marker)) = expression.kind
    else {
        panic!("selected function returns its integer marker")
    };
    i64::from(marker)
}

#[test]
fn unbound_reference_falls_through_exact_current_star_then_core() {
    for winner in 0..4 {
        let parameter = |layer| if layer < winner { "String" } else { "Boolean" };
        let exact_source = package(file(vec![candidate("choose", parameter(0), 1)]), "exactlib");
        let star_source = package(file(vec![candidate("choose", parameter(2), 3)]), "starlib");
        let core = core_with(vec![candidate("choose", parameter(3), 4)]);
        let mut user = consumer("choose", None, vec![ty_named("Boolean")]);
        user.declarations.push(candidate("choose", parameter(1), 2));
        user.imports = vec![exact("exactlib", "choose", None), star("starlib")];

        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("an inapplicable callable-reference layer falls through");
        assert_eq!(selected_marker(&output.export), i64::from(winner + 1));
    }
}

#[test]
fn exact_alias_selects_an_unbound_extension_reference() {
    let core = core_with(vec![struct_decl("Owner", Vec::new())]);
    let library = package(
        file(vec![extension("Owner", "decorate", "Boolean", 7)]),
        "api",
    );
    let mut user = consumer(
        "renamed",
        None,
        vec![ty_named("Owner"), ty_named("Boolean")],
    );
    user.imports.push(exact("api", "decorate", Some("renamed")));

    let output = lower_sources(vec![library, user], core)
        .expect("an exact alias preserves the unbound extension target");
    assert_eq!(selected_marker(&output.export), 7);
}

#[test]
fn duplicate_star_paths_to_one_reference_origin_are_deduplicated() {
    let library = package(file(vec![candidate("choose", "Boolean", 3)]), "api");
    let mut user = consumer("choose", None, vec![ty_named("Boolean")]);
    user.imports = vec![star("api"), star("api")];

    let output = lower_sources(vec![library, user], core_file())
        .expect("duplicate star paths do not create a reference ambiguity");
    assert_eq!(selected_marker(&output.export), 3);
}

#[test]
fn inaccessible_current_reference_does_not_hide_visible_star_candidate() {
    let mut private = candidate("choose", "Boolean", 2);
    let Decl::Function(function) = &mut private else {
        panic!("candidate builder returns a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let hidden = file(vec![private]);
    let library = package(file(vec![candidate("choose", "Boolean", 3)]), "api");
    let mut user = consumer("choose", None, vec![ty_named("Boolean")]);
    user.imports.push(star("api"));

    let output = lower_sources(vec![hidden, library, user], core_file())
        .expect("visibility is filtered before reference-layer applicability");
    assert_eq!(selected_marker(&output.export), 3);
}

#[test]
fn most_specific_reference_is_selected_within_one_import_layer() {
    let broad = package(file(vec![candidate("choose", "Any", 1)]), "broad");
    let narrow = package(file(vec![candidate("choose", "String", 2)]), "narrow");
    let mut user = consumer("choose", None, vec![ty_named("String")]);
    user.imports = vec![
        exact("broad", "choose", None),
        exact("narrow", "choose", None),
    ];

    let output = lower_sources(vec![broad, narrow, user], core_file())
        .expect("reference candidates use declaration forwarding for MSC");
    assert_eq!(selected_marker(&output.export), 2);
}

#[test]
fn reference_without_expected_type_requires_one_non_generic_candidate() {
    let broad = package(file(vec![candidate("choose", "Any", 1)]), "broad");
    let narrow = package(file(vec![candidate("choose", "String", 2)]), "narrow");
    let mut user = file(vec![fun(
        "main",
        vec![val("selected", reference(None, "choose"))],
    )]);
    user.imports = vec![
        exact("broad", "choose", None),
        exact("narrow", "choose", None),
    ];

    let errors = lower_sources(vec![broad, narrow, user], core_file())
        .expect_err("MSC cannot choose a callable reference without an expected type");
    assert!(
        errors.iter().any(|error| {
            error
                .message
                .contains("callable reference `::choose` is ambiguous")
                && error
                    .message
                    .contains("forms a complete non-generic function type")
        }),
        "{errors:?}"
    );
}

#[test]
fn applicable_reference_ambiguity_is_terminal_for_its_layer() {
    let left = package(file(vec![candidate("choose", "Boolean", 1)]), "left");
    let right = package(file(vec![candidate("choose", "Boolean", 2)]), "right");
    let mut user = consumer("choose", None, vec![ty_named("Boolean")]);
    user.declarations.push(candidate("choose", "Boolean", 3));
    user.imports = vec![
        exact("left", "choose", None),
        exact("right", "choose", None),
    ];

    let errors = lower_sources(vec![left, right, user], core_file())
        .expect_err("an applicable ambiguous exact layer cannot fall through");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("callable reference `::choose` is ambiguous")),
        "{errors:?}"
    );
}

#[test]
fn bound_reference_uses_member_then_exact_current_star_core_extensions() {
    for winner in 0..5 {
        let parameter = |layer| if layer < winner { "String" } else { "Boolean" };
        let methods = if winner == 0 {
            vec![method_expr(
                "choose",
                vec![("value", ty_named("Boolean"))],
                Some(ty_named("Int")),
                int_lit(1),
            )]
        } else {
            vec![method_expr(
                "choose",
                vec![("value", ty_named("String"))],
                Some(ty_named("Int")),
                int_lit(1),
            )]
        };
        let exact_source = package(
            file(vec![extension("Owner", "choose", parameter(1), 2)]),
            "exactlib",
        );
        let star_source = package(
            file(vec![extension("Owner", "choose", parameter(3), 4)]),
            "starlib",
        );
        let core = core_with(vec![
            struct_decl_methods("Owner", Vec::new(), methods),
            extension("Owner", "choose", parameter(4), 5),
        ]);
        let mut user = consumer(
            "choose",
            Some(call("Owner", Vec::new())),
            vec![ty_named("Boolean")],
        );
        user.declarations
            .push(extension("Owner", "choose", parameter(2), 3));
        user.imports = vec![exact("exactlib", "choose", None), star("starlib")];

        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("bound references use member and fixed extension layers");
        assert_eq!(selected_marker(&output.export), i64::from(winner + 1));
    }
}

#[test]
fn exact_object_and_companion_members_are_not_unbound_reference_targets() {
    let cases = [
        (
            object_with_method("Tools", "choose", 1),
            vec!["api", "Tools", "choose"],
        ),
        (
            class_with_companion_method("Host", "Factory", "choose", 2),
            vec!["api", "Host", "Factory", "choose"],
        ),
    ];
    for (declaration, path) in cases {
        let library = package(file(vec![declaration]), "api");
        let mut user = consumer("alias", None, vec![ty_named("Int")]);
        user.imports.push(exact_path(&path, Some("alias")));

        let errors = lower_sources(vec![library, user], core_file())
            .expect_err("an owner member cannot become an unbound `::alias` target");
        assert!(
            errors
                .iter()
                .any(|error| error.message == "unknown function `alias`"),
            "{errors:?}"
        );
    }
}

#[test]
fn filtered_exact_member_does_not_hide_current_top_level_reference() {
    let cases = [
        (
            object_with_method("Tools", "choose", 1),
            vec!["api", "Tools", "choose"],
        ),
        (
            class_with_companion_method("Host", "Factory", "choose", 2),
            vec!["api", "Host", "Factory", "choose"],
        ),
    ];
    for (declaration, path) in cases {
        let library = package(file(vec![declaration]), "api");
        let mut user = consumer("choose", None, vec![ty_named("Int")]);
        user.declarations.push(candidate("choose", "Int", 7));
        user.imports.push(exact_path(&path, Some("choose")));

        let output = lower_sources(vec![library, user], core_file())
            .expect("a filtered exact member leaves the layer empty and permits fallback");
        assert_eq!(selected_marker(&output.export), 7);
    }
}

#[test]
fn rejected_exact_object_members_do_not_hide_a_star_top_level_reference() {
    let Decl::Object(mut tools) = object_with_method("Tools", "choose", 1) else {
        panic!("object helper creates an object")
    };
    tools.members.push(ast::ClassMember::Function(method_expr(
        "choose",
        vec![("other", ty_named("Int"))],
        Some(ty_named("Int")),
        int_lit(2),
    )));
    let exact_source = package(file(vec![Decl::Object(tools)]), "api");
    let star_source = package(file(vec![candidate("choose", "String", 7)]), "starlib");
    let mut user = consumer("choose", None, vec![ty_named("Boolean")]);
    user.imports = vec![
        exact_path(&["api", "Tools", "choose"], Some("choose")),
        star("starlib"),
    ];

    let errors = lower_sources(vec![exact_source, star_source, user], core_file())
        .expect_err("the object methods are duplicate");
    assert!(errors.iter().any(|error| {
        error.message
            == "function `choose` in object `Tools` is already declared with the same signature"
    }));
    assert!(
        errors.iter().any(|error| {
            error.message.contains("callable reference `::choose`")
                && error.message.contains("fun choose(value: String): Int")
        }),
        "the filtered member role must leave the star reference visible: {errors:?}"
    );
}

#[test]
fn native_reference_ignores_exact_member_and_selects_current_top_level() {
    let library = package(
        file(vec![object_with_method("Tools", "callback", 1)]),
        "api",
    );
    let fun_ptr = ty_generic(
        "FunPtr",
        vec![ty_function(false, vec![ty_named("Int")], ty_named("Int"))],
    );
    let mut user = file(vec![
        no_gc(candidate("callback", "Int", 7)),
        fun(
            "main",
            vec![val_ty(
                "selected",
                Some(fun_ptr),
                reference(None, "callback"),
            )],
        ),
    ]);
    user.imports
        .push(exact_path(&["api", "Tools", "callback"], Some("callback")));

    let output = lower_sources(vec![library, user], core_file())
        .expect("native lookup only considers the legal current top-level declaration");
    let expression = local_init(body(&output.export, "main"), "selected");
    let hir::ExprKind::FunctionAddress(function) = expression.kind else {
        panic!("the selected native reference is a direct function address")
    };
    assert_eq!(output.export.functions[function].name, "callback");
    let hir::FunctionKind::User(body) = &output.export.functions[function].kind else {
        panic!("the callback has a source body")
    };
    assert!(matches!(
        return_value(&body.statements).kind,
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(7))
    ));
}

#[test]
fn native_reference_accepts_exact_alias_to_top_level_function() {
    let library = package(file(vec![no_gc(candidate("callback", "Int", 9))]), "api");
    let fun_ptr = ty_generic(
        "FunPtr",
        vec![ty_function(false, vec![ty_named("Int")], ty_named("Int"))],
    );
    let mut user = file(vec![fun(
        "main",
        vec![val_ty(
            "selected",
            Some(fun_ptr),
            reference(None, "renamed"),
        )],
    )]);
    user.imports.push(exact("api", "callback", Some("renamed")));

    let output = lower_sources(vec![library, user], core_file())
        .expect("native lookup retains an exact alias to a legal top-level function");
    let expression = local_init(body(&output.export, "main"), "selected");
    let hir::ExprKind::FunctionAddress(function) = expression.kind else {
        panic!("the exact alias lowers directly to a native function address")
    };
    assert_eq!(output.export.functions[function].name, "callback");
}
