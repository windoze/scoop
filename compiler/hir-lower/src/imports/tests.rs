use super::*;
mod type_lookup;
mod value_lookup;
use crate::tests::{file, ident, sp};
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
    ast::ImportSyntax::Star {
        exposure: exposure(public),
        namespace: path(segments),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: ast::Span::new(0, 90),
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
    let request = ast::Stage1RequestId::from_raw(19);
    let files = [
        file(Vec::new()),
        package(file(Vec::new()), &["api"]),
        package(file(Vec::new()), &["api", "child"]),
    ];
    let mut lowerer = Lowerer::new().with_intrinsic_sources(
        (0..3)
            .map(|index| SourceProvider {
                provider: hir::IntrinsicProviderId::from_raw(1),
                kind: SourceKind::CurrentUnit,
                visibility_source: hir::VisibilitySource::CurrentUnit(
                    ast::Stage1SourceHandle::new(request, index),
                ),
                name: "same.scoop".to_string(),
                source: String::new(),
            })
            .collect(),
        IntrinsicDeclarationPolicy::CoreOnly,
    );
    lowerer
        .top_level_namespaces
        .initialize_sources([SourceKind::CurrentUnit; 3], &files);
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
    let source = match lowerer.visibility_file(file).source {
        hir::VisibilitySource::CurrentUnit(handle) => handle,
        _ => unreachable!(),
    };
    let domain = lowerer.top_level_domain(
        if private {
            hir::DeclaredVisibility::Private
        } else {
            hir::DeclaredVisibility::Internal
        },
        file,
    );
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
    let (mut lowerer, api, _) = lowerer();
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
    for import in [
        exact(&["api", "secret"], None, true),
        exact(&["api", "secret"], Some("alias"), true),
        star(&["api"], true),
        star(&["api", "child"], true),
    ] {
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
        ] {
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
            assert_eq!(lowerer.diagnostics[0].span.unwrap().start, 20);
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

fn lower_sources(sources: Vec<ast::SourceFile>) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower_sources_with_core(sources, crate::tests::core_file())
}

fn lower_sources_with_core(
    sources: Vec<ast::SourceFile>,
    core: ast::SourceFile,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let request = ast::Stage1RequestId::from_raw(100);
    let mut parsed = sources.into_iter().enumerate().map(|(index, source)| {
        ast::ParsedSource::new(
            ast::Stage1SourceHandle::new(request, u32::try_from(index).expect("test source index")),
            source,
        )
    });
    let parsed = ast::AllParsedSources::try_new(
        request,
        ast::NonEmptyVec::new(
            parsed.next().expect("test sources nonempty"),
            parsed.collect(),
        ),
    )
    .expect("test sources belong to one request");
    let input = crate::Stage1CompilationInput::new(
        vec![crate::ProviderSource {
            source: &core,
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::Stage1SourceDetails {
            display_locator: "same.scoop",
            source_text: "",
        },
    );
    crate::lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
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
