use super::*;
mod type_lookup;
mod value_lookup;
use crate::tests::{
    call, core_source_identity, enum_decl, field, file, fun, ident, identified_test_sources, sp,
    stmt, str_lit, test_source_identity, ty_named, val, var, variant_positional, variant_unit,
};
use crate::{IntrinsicDeclarationPolicy, Lowerer, SourceKind, SourceProvider};

fn path(segments: &[&str]) -> ast::QualifiedNameSyntax {
    let mut offset = 20;
    let mut identifier = |text: &str| {
        let start = offset;
        offset += u32::try_from(text.len()).expect("test identifier length") + 1;
        ast::Ident {
            text: text.to_string(),
            span: ast::Span::new(start, offset - 1),
        }
    };
    let first = identifier(segments[0]);
    let rest = segments[1..]
        .iter()
        .map(|name| {
            let identifier = identifier(name);
            ast::QualifiedNameTailSyntax {
                dot_span: ast::Span::new(identifier.span.start - 1, identifier.span.start),
                identifier,
            }
        })
        .collect();
    ast::QualifiedNameSyntax {
        first,
        rest,
        span: ast::Span::new(20, offset - 1),
    }
}

fn exact(segments: &[&str], alias: Option<&str>, public: bool) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: exposure(public),
        selector: path(segments),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: ast::Span::new(7, 13),
        span: ast::Span::new(0, 90),
    }
}

fn exposure(public: bool) -> ast::ImportExposureSyntax {
    if public {
        ast::ImportExposureSyntax::PublicReexport {
            public_keyword_span: ast::Span::new(0, 6),
        }
    } else {
        ast::ImportExposureSyntax::Local
    }
}

fn star(segments: &[&str], public: bool) -> ast::ImportSyntax {
    let namespace = path(segments);
    let terminal_dot_span = ast::Span::new(namespace.span.end, namespace.span.end + 1);
    let star_span = ast::Span::new(terminal_dot_span.end, terminal_dot_span.end + 1);
    ast::ImportSyntax::Star {
        exposure: exposure(public),
        namespace,
        import_keyword_span: sp(),
        terminal_dot_span,
        star_span,
        span: ast::Span::new(0, 90),
    }
}

fn selector_span(import: &ast::ImportSyntax) -> ast::Span {
    match import {
        ast::ImportSyntax::Exact { selector, .. } => selector.span,
        ast::ImportSyntax::Star {
            namespace,
            star_span,
            ..
        } => ast::Span::new(namespace.span.start, star_span.end),
    }
}

fn package(mut source: ast::SourceFile, segments: &[&str]) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: path(segments),
        span: sp(),
    };
    source
}

fn lowerer() -> (Lowerer, PackageId, PackageId) {
    let files = [
        file(Vec::new()),
        package(file(Vec::new()), &["api"]),
        package(file(Vec::new()), &["api", "child"]),
        package(file(Vec::new()), &["api", "empty"]),
    ];
    let mut lowerer = Lowerer::new().with_intrinsic_sources(
        [
            "src/00-root.scoop",
            "src/01-api.scoop",
            "src/02-api-child.scoop",
            "src/03-api-empty.scoop",
        ]
        .into_iter()
        .map(|path| SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(1),
            kind: SourceKind::CurrentUnit,
            identity: test_source_identity(path),
            name: "same.scoop".to_string(),
            source: String::new(),
        })
        .collect(),
        IntrinsicDeclarationPolicy::CoreOnly,
    );
    lowerer
        .top_level_namespaces
        .initialize_sources([SourceKind::CurrentUnit; 4], &files);
    let (api, _) = lowerer
        .top_level_namespaces
        .longest_package_prefix(&[ident("api")]);
    let (child, _) = lowerer
        .top_level_namespaces
        .longest_package_prefix(&[ident("api"), ident("child")]);
    (lowerer, api, child)
}

fn function(
    surface: &mut CurrentUnitImports,
    lowerer: &Lowerer,
    namespace: ResolvedNamespace,
    name: &str,
    file: usize,
    index: u32,
    private: bool,
) -> CurrentUnitBindingId {
    function_with_visibility(
        surface,
        lowerer,
        namespace,
        name,
        file,
        index,
        if private {
            hir::DeclaredVisibility::Private
        } else {
            hir::DeclaredVisibility::Internal
        },
    )
}

fn function_with_visibility(
    surface: &mut CurrentUnitImports,
    lowerer: &Lowerer,
    namespace: ResolvedNamespace,
    name: &str,
    file: usize,
    index: u32,
    visibility: hir::DeclaredVisibility,
) -> CurrentUnitBindingId {
    let source = lowerer.visibility_file(file);
    let domain = lowerer.top_level_domain(visibility, file);
    surface.insert(
        namespace,
        CurrentUnitBinding {
            target: CurrentUnitTarget::Function(hir::FunctionId::from_raw(index.into())),
            source,
            file,
            span: ast::Span::new(index * 10, index * 10 + 3),
            access: hir::EffectiveLookupDomain(domain),
            name: name.to_string(),
        },
    )
}

#[test]
fn exact_alias_and_star_retain_overloads_and_deduplicate_each_layer() {
    let (mut lowerer, api, _) = lowerer();
    let mut surface = CurrentUnitImports::default();
    let first = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "choose",
        1,
        1,
        false,
    );
    let second = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "choose",
        2,
        2,
        false,
    );
    let mut syntax = file(Vec::new());
    syntax.imports = vec![
        exact(&["api", "choose"], Some("select"), false),
        exact(&["api", "choose"], Some("select"), false),
        star(&["api"], false),
        star(&["api"], false),
    ];
    for reverse in [false, true] {
        if reverse {
            syntax.imports.reverse();
        }
        let resolved = surface.resolve_file(&mut lowerer, &syntax);
        assert!(lowerer.diagnostics.is_empty());
        assert_eq!(resolved.exact[0].targets.len(), 2);
        surface.files = vec![resolved];
        let alias_layers = surface.layers(
            0,
            crate::namespace::TopLevelNamespaces::root_package(),
            "select",
        );
        assert_eq!(
            alias_layers
                .iter()
                .map(|layer| layer.kind)
                .collect::<Vec<_>>(),
            [
                ImportLookupLayer::Exact,
                ImportLookupLayer::CurrentPackage(
                    crate::namespace::TopLevelNamespaces::root_package()
                ),
                ImportLookupLayer::Star,
                ImportLookupLayer::CorePrelude
            ]
        );
        assert_eq!(alias_layers[0].bindings, [first, second]);
        assert!(alias_layers[2].bindings.is_empty());
        let original_layers = surface.layers(0, api, "choose");
        assert_eq!(original_layers[1].bindings, [first, second]);
        assert_eq!(original_layers[2].bindings, [first, second]);
    }
}

#[test]
fn exact_private_failure_has_a_declaration_note_and_commits_nothing() {
    let (mut lowerer, api, _) = lowerer();
    let mut surface = CurrentUnitImports::default();
    function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "secret",
        1,
        7,
        true,
    );
    let mut syntax = file(Vec::new());
    syntax.imports = vec![
        exact(&["api", "secret"], Some("leaked"), false),
        star(&["api"], false),
    ];
    let resolved = surface.resolve_file(&mut lowerer, &syntax);
    assert!(resolved.exact.is_empty());
    assert!(resolved.stars[0].snapshot.is_empty());
    assert_eq!(lowerer.diagnostics.len(), 1);
    let error = &lowerer.diagnostics[0];
    assert_eq!(
        error.span,
        Some(path(&["api", "secret"]).segments().last().unwrap().span)
    );
    assert_eq!(error.notes.len(), 1);
    assert_eq!(error.notes[0].file, 1);
    assert_eq!(error.notes[0].span, ast::Span::new(70, 73));
    lowerer.current_file = 1;
    lowerer.diagnostics.clear();
    let local = surface.resolve_file(&mut lowerer, &syntax);
    assert_eq!(local.exact[0].targets.len(), 1);
    assert_eq!(local.stars[0].snapshot["secret"].len(), 1);
    assert!(lowerer.diagnostics.is_empty());
}

#[test]
fn public_gate_precedes_visibility_and_never_publishes_local_bindings() {
    let (mut lowerer, api, child) = lowerer();
    let mut surface = CurrentUnitImports::default();
    function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "secret",
        1,
        1,
        true,
    );
    function_with_visibility(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(child),
        "published",
        2,
        2,
        hir::DeclaredVisibility::Public,
    );

    let mut local_star = file(Vec::new());
    local_star.imports.push(star(&["api", "child"], false));
    let locally_resolved = surface.resolve_file(&mut lowerer, &local_star);
    assert_eq!(locally_resolved.stars[0].snapshot["published"].len(), 1);
    assert!(lowerer.diagnostics.is_empty());

    for (current_file, import) in [
        (0, exact(&["api", "secret"], None, true)),
        (0, exact(&["api", "secret"], Some("alias"), true)),
        (1, exact(&["api", "secret"], None, true)),
        (0, exact(&["api", "child", "published"], None, true)),
        (0, star(&["api"], true)),
        (0, star(&["api", "child"], true)),
        (0, star(&["api", "empty"], true)),
    ] {
        lowerer.current_file = current_file;
        lowerer.diagnostics.clear();
        let mut syntax = file(Vec::new());
        syntax.imports.push(import);
        let resolved = surface.resolve_file(&mut lowerer, &syntax);
        assert!(resolved.exact.is_empty() && resolved.stars.is_empty());
        assert_eq!(lowerer.diagnostics.len(), 1);
        assert_eq!(
            lowerer.diagnostics[0].message,
            "public import requires a direct dependency target"
        );
        assert_eq!(lowerer.diagnostics[0].span, Some(ast::Span::new(0, 6)));
        assert!(lowerer.diagnostics[0].notes.is_empty());
    }
}

#[test]
fn unknown_and_external_public_selectors_keep_unavailable_diagnostics() {
    let (mut lowerer, _, _) = lowerer();
    let surface = CurrentUnitImports::default();
    for public in [false, true] {
        for import in [
            exact(&["scoop", "core", "Int"], None, public),
            star(&["scoop", "core"], public),
            exact(&["unknown", "Thing"], None, public),
            star(&["unknown", "space"], public),
        ] {
            let expected_span = selector_span(&import);
            lowerer.diagnostics.clear();
            let mut syntax = file(Vec::new());
            syntax.imports.push(import);
            let resolved = surface.resolve_file(&mut lowerer, &syntax);
            assert!(resolved.exact.is_empty() && resolved.stars.is_empty());
            assert_eq!(lowerer.diagnostics.len(), 1);
            assert_eq!(
                lowerer.diagnostics[0].message,
                "import target is not available in the current compilation unit"
            );
            assert_eq!(lowerer.diagnostics[0].span, Some(expected_span));
        }
    }
}

#[test]
fn star_is_nonrecursive_and_exact_cannot_import_a_package() {
    let (mut lowerer, api, child) = lowerer();
    let mut surface = CurrentUnitImports::default();
    function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(child),
        "childOnly",
        2,
        1,
        false,
    );
    let mut syntax = file(Vec::new());
    syntax.imports = vec![star(&["api"], false), exact(&["api"], None, false)];
    let resolved = surface.resolve_file(&mut lowerer, &syntax);
    assert!(resolved.exact.is_empty());
    assert_eq!(resolved.stars[0].namespace, ResolvedNamespace::Package(api));
    assert!(resolved.stars[0].snapshot.is_empty());
    assert_eq!(
        lowerer.diagnostics[0].message,
        "exact import requires an importable binding, not a package namespace"
    );
}

#[test]
fn static_paths_use_typed_edges_and_do_not_fall_back_from_longest_package() {
    let (mut lowerer, api, _) = lowerer();
    let mut surface = CurrentUnitImports::default();
    let host = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "Host",
        1,
        1,
        false,
    );
    let static_owner = StaticNamespace::Class(hir::ClassId::from_raw(0.into()));
    surface.static_targets.insert(host, static_owner);
    let member = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Static(static_owner),
        "factory",
        1,
        2,
        false,
    );
    let shadowed = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "child",
        1,
        3,
        false,
    );
    surface.static_targets.insert(shadowed, static_owner);
    let mut syntax = file(Vec::new());
    syntax.imports = vec![
        exact(&["api", "Host", "factory"], None, false),
        star(&["api", "Host"], false),
        exact(&["api", "child", "factory"], None, false),
    ];
    let resolved = surface.resolve_file(&mut lowerer, &syntax);
    assert_eq!(resolved.exact.len(), 1);
    assert_eq!(resolved.exact[0].targets.first().binding, member);
    assert_eq!(
        resolved.stars[0].snapshot["factory"].first().binding,
        member
    );
    assert_eq!(lowerer.diagnostics.len(), 1);
    assert_eq!(
        lowerer.diagnostics[0].message,
        "import target is not available in the current compilation unit"
    );
}

#[test]
fn ambiguous_star_namespace_diagnostic_covers_the_complete_selector() {
    let (mut lowerer, api, _) = lowerer();
    let mut surface = CurrentUnitImports::default();
    let first = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "Host",
        1,
        1,
        false,
    );
    let second = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "Host",
        2,
        2,
        false,
    );
    surface.static_targets.insert(
        first,
        StaticNamespace::Class(hir::ClassId::from_raw(1.into())),
    );
    surface.static_targets.insert(
        second,
        StaticNamespace::Class(hir::ClassId::from_raw(2.into())),
    );
    let import = star(&["api", "Host"], false);
    let expected_span = selector_span(&import);
    let mut syntax = file(Vec::new());
    syntax.imports.push(import);

    let resolved = surface.resolve_file(&mut lowerer, &syntax);
    assert!(resolved.stars.is_empty());
    assert_eq!(lowerer.diagnostics.len(), 1);
    assert_eq!(
        lowerer.diagnostics[0].message,
        "import namespace is ambiguous in the current compilation unit"
    );
    assert_eq!(lowerer.diagnostics[0].span, Some(expected_span));
}

fn lower_sources(sources: Vec<ast::SourceFile>) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower_sources_with_core(sources, crate::tests::core_file())
}

fn lower_sources_with_core(
    sources: Vec<ast::SourceFile>,
    core: ast::SourceFile,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let parsed = identified_test_sources(sources);
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "same.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    crate::lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn invalid_raw_string_property(name: &str) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
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
        ty: ty_named("String"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(str_lit("invalid raw image")),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

#[test]
fn failed_property_materialization_does_not_stop_independent_body_diagnostics() {
    let errors = lower_sources(vec![file(vec![
        invalid_raw_string_property("broken"),
        fun("main", vec![stmt(call("missing", Vec::new()))]),
    ])])
    .expect_err("both the declaration and independent body use must be diagnosed");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("has no valid all-zero initial image")),
        "unexpected diagnostics: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("unknown function `missing`")),
        "body lowering stopped after the declaration error: {messages:?}"
    );
}

#[test]
fn unmaterialized_exact_value_blocks_core_fallback_without_becoming_a_target() {
    let producer = package(
        file(vec![
            fun("println", Vec::new()),
            invalid_raw_string_property("println"),
        ]),
        &["api"],
    );
    let mut consumer = file(vec![fun("main", vec![stmt(call("println", Vec::new()))])]);
    consumer.imports = vec![exact(&["api", "println"], None, false)];

    let errors = lower_sources(vec![producer, consumer])
        .expect_err("an unmaterialized exact target must not fall through to core");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("has no valid all-zero initial image")),
        "unexpected diagnostics: {messages:?}"
    );
    assert!(
        messages.contains(&"function `println` is not accessible here"),
        "the failed exact target silently downgraded to the core prelude: {messages:?}"
    );
}

#[test]
fn failed_variant_does_not_shift_a_later_source_variant_identity() {
    let errors = lower_sources(vec![file(vec![
        enum_decl(
            "Status",
            Vec::new(),
            vec![
                variant_positional("Broken", vec![ty_named("Missing")]),
                variant_unit("Good"),
            ],
        ),
        fun("main", vec![val("good", field(var("Status"), "Good"))]),
    ])])
    .expect_err("the invalid first variant field type must be diagnosed");
    assert_eq!(errors.len(), 1, "unexpected diagnostics: {errors:?}");
    assert_eq!(errors[0].message, "unknown type `Missing`");
}

fn constant(name: &str) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: crate::tests::ty_named("Int"),
        body: ast::PropertyBodySyntax::Const(Box::new(crate::tests::int_lit(7))),
        span: sp(),
    }
}

fn method(name: &str) -> ast::FunctionDecl {
    let ast::Decl::Function(function) = crate::tests::fun(name, Vec::new()) else {
        unreachable!()
    };
    function
}

fn declared_function(name: &str, visibility: ast::DeclaredVisibility) -> ast::Decl {
    let mut declaration = crate::tests::fun(name, Vec::new());
    let ast::Decl::Function(function) = &mut declaration else {
        unreachable!("the function builder returns a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility,
        span: sp(),
    };
    declaration
}

fn with_import(import: ast::ImportSyntax) -> ast::SourceFile {
    let mut source = file(Vec::new());
    source.imports.push(import);
    source
}

#[test]
fn pipeline_materializes_importable_nominals_aliases_properties_variants_and_static_members() {
    use crate::tests::{
        class_decl, enum_decl, fun, interface_decl, struct_decl, ty_named, variant_unit,
    };
    let ast::Decl::Struct(nested) = struct_decl("Nested", Vec::new()) else {
        unreachable!()
    };
    let object = ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident("Tools"),
        supertypes: Vec::new(),
        members: vec![
            ast::ClassMember::Function(method("answer")),
            ast::ClassMember::StoredProperty(constant("version")),
            ast::ClassMember::Nested(Box::new(ast::NestedNominalDecl::Struct(Box::new(nested)))),
        ],
        span: sp(),
    });
    let ast::Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        vec![method("instanceOnly")],
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Companion(Box::new(
        ast::CompanionObjectDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            name: ast::CompanionNameSyntax::Named(ident("Factory")),
            supertypes: Vec::new(),
            members: vec![
                ast::ClassMember::Function(method("make")),
                ast::ClassMember::StoredProperty(constant("revision")),
                ast::ClassMember::Companion(Box::new(ast::CompanionObjectDecl {
                    annotations: Vec::new(),
                    visibility: ast::VisibilitySyntax::Omitted,
                    name: ast::CompanionNameSyntax::Named(ident("Inner")),
                    supertypes: Vec::new(),
                    members: vec![ast::ClassMember::Function(method("deep"))],
                    span: sp(),
                })),
            ],
            span: sp(),
        },
    )));
    let mut extension = method("extension");
    extension.receiver_ty = Some(ty_named("Int"));
    let declarations = package(
        file(vec![
            struct_decl("Item", Vec::new()),
            interface_decl("Contract", Vec::new()),
            enum_decl("State", Vec::new(), vec![variant_unit("Ready")]),
            ast::Decl::TypeAlias(ast::TypeAliasDecl {
                visibility: ast::VisibilitySyntax::Omitted,
                name: ident("Alias"),
                target: ty_named("Item"),
                span: sp(),
            }),
            ast::Decl::Global(constant("answer")),
            fun("helper", Vec::new()),
            ast::Decl::Function(extension),
            object,
            ast::Decl::Class(host),
        ]),
        &["api"],
    );
    let mut consumer = file(vec![fun("main", Vec::new())]);
    consumer.imports = [
        vec!["api", "Item"],
        vec!["api", "Contract"],
        vec!["api", "State"],
        vec!["api", "State", "Ready"],
        vec!["api", "Alias"],
        vec!["api", "answer"],
        vec!["api", "helper"],
        vec!["api", "extension"],
        vec!["api", "Tools"],
        vec!["api", "Tools", "answer"],
        vec!["api", "Tools", "version"],
        vec!["api", "Tools", "Nested"],
        vec!["api", "Host", "make"],
        vec!["api", "Host", "revision"],
        vec!["api", "Host", "Factory", "make"],
        vec!["api", "Host", "Companion", "make"],
        vec!["api", "Host", "Factory", "deep"],
    ]
    .iter()
    .map(|selector| exact(selector, None, false))
    .collect();
    consumer.imports.extend([
        star(&["api", "Tools"], false),
        star(&["api", "Host"], false),
        star(&["api", "State"], false),
    ]);
    lower_sources(vec![declarations.clone(), consumer.clone()])
        .expect("all importable source categories materialize typed targets");
    lower_sources(vec![consumer.clone(), declarations.clone()])
        .expect("source order does not affect binding materialization");
    consumer.imports = vec![exact(&["api", "Host", "deep"], None, false)];
    let errors = lower_sources(vec![declarations.clone(), consumer.clone()])
        .expect_err("companion forwarding is one typed namespace edge");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "import target is not available in the current compilation unit"
    );
    consumer.imports = vec![exact(&["api", "Host", "instanceOnly"], None, false)];
    let errors = lower_sources(vec![declarations, consumer])
        .expect_err("instance methods are not importable static members");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "import target is not available in the current compilation unit"
    );
}

#[test]
fn cross_file_ordinary_imports_accept_public_bindings() {
    use crate::tests::{call, fun, stmt};

    let declarations = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["api"],
    );
    for import in [
        exact(&["api", "published"], None, false),
        star(&["api"], false),
    ] {
        let mut consumer = file(vec![fun(
            "usePublished",
            vec![stmt(call("published", Vec::new()))],
        )]);
        consumer.imports.push(import);
        lower_sources(vec![declarations.clone(), consumer])
            .expect("ordinary exact and star imports may cross files to a public binding");
    }
}

#[test]
fn import_failure_classes_return_no_hir_output() {
    let public_api = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["api"],
    );
    let private_api = package(
        file(vec![declared_function(
            "secret",
            ast::DeclaredVisibility::Private,
        )]),
        &["privateApi"],
    );
    let empty_api = package(file(Vec::new()), &["emptyApi"]);
    let mut own_private = file(vec![declared_function(
        "ownSecret",
        ast::DeclaredVisibility::Private,
    )]);
    own_private.imports.push(exact(&["ownSecret"], None, true));

    let cases = vec![
        (
            "public exact of a public current-unit target",
            vec![
                public_api.clone(),
                with_import(exact(&["api", "published"], None, true)),
            ],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public star of an accessible nonempty current-unit namespace",
            vec![public_api, with_import(star(&["api"], true))],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public star of an empty current-unit namespace",
            vec![empty_api, with_import(star(&["emptyApi"], true))],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public exact of a same-file private target",
            vec![own_private],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "ordinary exact of a cross-file private target",
            vec![
                private_api,
                with_import(exact(&["privateApi", "secret"], None, false)),
            ],
            "import target is not accessible from this source location",
            1,
        ),
        (
            "ordinary star of an unavailable namespace",
            vec![with_import(star(&["unknown", "space"], false))],
            "import target is not available in the current compilation unit",
            0,
        ),
        (
            "public star of an unavailable namespace",
            vec![with_import(star(&["unknown", "space"], true))],
            "import target is not available in the current compilation unit",
            0,
        ),
        (
            "public star of the explicit core namespace",
            vec![with_import(star(&["scoop", "core"], true))],
            "import target is not available in the current compilation unit",
            0,
        ),
    ];

    for (case, sources, expected_message, expected_notes) in cases {
        let errors = lower_sources(sources).expect_err(case);
        assert_eq!(errors.len(), 1, "{case}: {errors:#?}");
        assert_eq!(errors[0].message, expected_message, "{case}");
        assert_eq!(errors[0].notes.len(), expected_notes, "{case}");
    }
}

#[test]
fn non_prelude_core_variant_is_not_a_bare_name() {
    use crate::tests::{enum_decl, fun, val, var, variant_unit};

    let mut core = crate::tests::core_file();
    core.declarations.push(enum_decl(
        "CoreOnlyState",
        Vec::new(),
        vec![variant_unit("CoreOnlyReady")],
    ));
    crate::tests::make_core_public(&mut core);
    let user = file(vec![fun(
        "probe",
        vec![val("chosen", var("CoreOnlyReady"))],
    )]);

    let errors = lower_sources_with_core(vec![user], core)
        .expect_err("only typed core-prelude variants may be used as bare names");
    assert!(errors.iter().any(|error| {
        error
            .message
            .starts_with("unknown variable `CoreOnlyReady`")
    }));
}

#[test]
fn failed_import_stops_before_unrelated_body_lowering() {
    use crate::tests::{call, fun, stmt};
    let declarations = package(file(vec![fun("helper", Vec::new())]), &["api"]);
    let mut consumer = file(vec![fun(
        "main",
        vec![stmt(call("unrelatedMissingFunction", Vec::new()))],
    )]);
    consumer
        .imports
        .push(exact(&["api", "helper"], Some("alias"), true));
    let errors = lower_sources(vec![declarations, consumer])
        .expect_err("failed public import cannot produce HIR");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "public import requires a direct dependency target"
    );
}

#[test]
fn public_import_pipeline_does_not_downgrade_to_an_ordinary_import() {
    let public_api = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["publicApi"],
    );
    let private_api = package(
        file(vec![declared_function(
            "secret",
            ast::DeclaredVisibility::Private,
        )]),
        &["privateApi"],
    );

    let mut aliased_consumer = file(vec![fun(
        "useAliased",
        vec![stmt(call("renamed", Vec::new()))],
    )]);
    aliased_consumer
        .imports
        .push(exact(&["publicApi", "published"], Some("renamed"), true));

    let mut private_exact_consumer = file(vec![fun(
        "usePrivateExact",
        vec![stmt(call("secret", Vec::new()))],
    )]);
    private_exact_consumer
        .imports
        .push(exact(&["privateApi", "secret"], None, true));

    let mut private_star_consumer = file(vec![fun(
        "usePrivateStar",
        vec![stmt(call("secret", Vec::new()))],
    )]);
    private_star_consumer
        .imports
        .push(star(&["privateApi"], true));

    for (case, sources) in [
        ("public exact alias", vec![public_api, aliased_consumer]),
        (
            "cross-file private public exact",
            vec![private_api.clone(), private_exact_consumer],
        ),
        (
            "private-only public star",
            vec![private_api, private_star_consumer],
        ),
    ] {
        let errors = lower_sources(sources).expect_err(case);
        assert_eq!(errors.len(), 1, "{case}: {errors:#?}");
        assert_eq!(
            errors[0].message, "public import requires a direct dependency target",
            "{case}: the public form must neither bind locally nor degrade to ordinary lookup"
        );
        assert_eq!(errors[0].span, Some(ast::Span::new(0, 6)), "{case}");
    }
}
