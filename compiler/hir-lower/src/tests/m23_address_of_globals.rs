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

fn core_with_global(marker: i64) -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.push(raw_global("cell", marker));
    make_core_public(&mut core);
    core
}

fn raw_global(name: &str, value: i64) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: vec![ast::Annotation {
            name: ident("Global"),
            args: Vec::new(),
            span: sp(),
        }],
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(int_lit(value)),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn ordinary_property(name: &str, value: i64) -> Decl {
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
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(value))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    })
}

fn private(mut declaration: Decl) -> Decl {
    let Decl::Global(property) = &mut declaration else {
        panic!("test visibility helper expects a property")
    };
    property.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    declaration
}

fn safety_block(statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::SafetyBlock {
            mode: ast::SafetyMode::Unsafe,
            block: block(statements),
        },
        span: sp(),
    }
}

fn consumer(imports: Vec<ast::ImportSyntax>, mut declarations: Vec<Decl>) -> ast::SourceFile {
    declarations.push(fun(
        "main",
        vec![safety_block(vec![val(
            "pointer",
            typed_call("addressOf", vec![ty_named("Int")], vec![var("cell")]),
        )])],
    ));
    let mut source = file(declarations);
    source.imports = imports;
    source
}

fn addressed_marker(output: &hir::Output) -> i64 {
    let main = output
        .export
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "main").then_some(function))
        .expect("the test program has main");
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main has a source body")
    };
    let hir::ExprKind::AddressOf(hir::Place::Global(global)) = local_init(body, "pointer").kind
    else {
        panic!("addressOf(global) lowers to a typed global place")
    };
    let hir::GlobalStorage::Local {
        initializer: hir::HirConstantImage::Integer(hir::HirIntegerConstant::Signed32(value)),
        ..
    } = output.export.globals[global].storage
    else {
        panic!("the selected test global retains its integer image")
    };
    i64::from(value)
}

#[test]
fn address_of_global_uses_exact_alias_current_star_then_core() {
    let exact_library = package(file(vec![raw_global("source", 1)]), "exactlib");
    let star_library = package(file(vec![raw_global("cell", 3)]), "starlib");
    let exact_user = consumer(
        vec![exact("exactlib", "source", Some("cell")), star("starlib")],
        vec![raw_global("cell", 2)],
    );
    let output = lower_sources(
        vec![exact_library, star_library, exact_user],
        core_with_global(4),
    )
    .expect("an exact alias names its typed raw global");
    assert_eq!(addressed_marker(&output), 1);

    let star_library = package(file(vec![raw_global("cell", 3)]), "starlib");
    let current_user = consumer(vec![star("starlib")], vec![raw_global("cell", 2)]);
    let output = lower_sources(vec![star_library, current_user], core_with_global(4))
        .expect("a current-package raw global precedes star and core");
    assert_eq!(addressed_marker(&output), 2);

    let star_library = package(file(vec![raw_global("cell", 3)]), "starlib");
    let star_user = consumer(vec![star("starlib")], Vec::new());
    let output = lower_sources(vec![star_library, star_user], core_with_global(4))
        .expect("a star-imported raw global precedes core");
    assert_eq!(addressed_marker(&output), 3);

    let output = lower_sources(vec![consumer(Vec::new(), Vec::new())], core_with_global(4))
        .expect("core raw storage remains the final value layer");
    assert_eq!(addressed_marker(&output), 4);
}

#[test]
fn higher_non_native_property_blocks_lower_raw_storage() {
    let exact_library = package(file(vec![ordinary_property("source", 1)]), "exactlib");
    let user = consumer(
        vec![exact("exactlib", "source", Some("cell"))],
        vec![raw_global("cell", 2)],
    );
    let errors = lower_sources(vec![exact_library, user], core_with_global(4))
        .expect_err("a selected non-native property cannot fall through by representation");
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .starts_with("`addressOf` argument must be an addressable")),
        "{errors:?}"
    );
}

#[test]
fn same_layer_raw_global_ambiguity_is_terminal_and_lists_origins() {
    let left = package(file(vec![raw_global("left", 1)]), "left");
    let right = package(file(vec![raw_global("right", 2)]), "right");
    let user = consumer(
        vec![
            exact("left", "left", Some("cell")),
            exact("right", "right", Some("cell")),
        ],
        vec![raw_global("cell", 3)],
    );
    let errors = lower_sources(vec![left, right, user], core_with_global(4))
        .expect_err("distinct exact raw globals are ambiguous and cannot fall through");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| {
            diagnostic.message == "value `cell` is ambiguous in the exact import layer"
        })
        .unwrap_or_else(|| panic!("missing exact-layer ambiguity: {errors:?}"));
    assert_eq!(diagnostic.notes.len(), 2, "{diagnostic:?}");
    assert!(
        diagnostic
            .notes
            .iter()
            .all(|note| note.message == "candidate declared here"),
        "{diagnostic:?}"
    );
}

#[test]
fn inaccessible_current_value_allows_star_global_fallback() {
    let hidden = file(vec![private(raw_global("cell", 2))]);
    let library = package(file(vec![raw_global("cell", 3)]), "starlib");
    let user = consumer(vec![star("starlib")], Vec::new());
    let output = lower_sources(vec![hidden, library, user], core_with_global(4))
        .expect("an inaccessible current value does not shadow a visible star global");
    assert_eq!(addressed_marker(&output), 3);
}
