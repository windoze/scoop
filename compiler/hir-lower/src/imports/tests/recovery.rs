use super::*;

#[test]
fn recovered_file_context_uses_normal_packages_imports_and_diagnostic_spans() {
    let source = "package api\nimport api.choose as select\nfun editing() { value. }";
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    let header = parsed
        .declaration_data()
        .source
        .as_ref()
        .unwrap()
        .header(ast::Span::new(0, source.len() as u32))
        .unwrap();
    let (mut lowerer, api, _) = lowerer();
    let cone = lowerer.current_cone();
    let mut namespaces = crate::namespace::TopLevelNamespaces::default();
    namespaces.initialize_sources(
        [(cone, scoop_identity::SourceRole::Production)],
        cone,
        &[header.into()],
    );
    let crate::namespace::TopLevelLookupLayer::CurrentPackage(package) =
        namespaces.source_namespace(0)
    else {
        panic!("a current source has its own package");
    };
    assert_eq!(namespaces.package_segments(package), ["api"]);

    let mut surface = CurrentUnitImports::default();
    let target = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "choose",
        1,
        1,
        false,
    );
    let resolved = surface.resolve_file_with_world(&mut lowerer, &header, None);
    assert!(lowerer.diagnostics.is_empty());
    assert_eq!(resolved.exact[0].local_name, "select");
    assert_eq!(resolved.exact[0].targets.len(), 1);
    assert_eq!(
        resolved.exact[0].targets.first().current_binding(),
        Some(target)
    );

    let mut empty = CurrentUnitImports::default();
    empty.resolve_file_with_world(&mut lowerer, &header, None);
    let ast::ImportSyntax::Exact { selector, .. } = &header.imports[0] else {
        panic!("the source has an exact import");
    };
    assert_eq!(lowerer.diagnostics.len(), 1);
    assert_eq!(lowerer.diagnostics[0].span, Some(selector.span));
}

#[test]
fn incomplete_package_or_import_cannot_become_a_complete_file_context() {
    for source in ["package\nfun editing() {}", "import api.\nfun editing() {}"] {
        let parsed = scoop_parser::parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert!(
            parsed
                .declaration_data()
                .source
                .as_ref()
                .unwrap()
                .header(ast::Span::new(0, source.len() as u32))
                .is_none()
        );
    }
}

#[test]
fn dependency_only_packages_keep_unknown_source_names_in_exact_and_star_imports() {
    let (mut lowerer, _, _) = lowerer();
    let fixture = super::dependency_fixture::DependencyWorldFixture::with_nested_type(
        &["dependency", "api"],
        "Outer",
        "Nested",
    );
    let world = fixture.world(lowerer.current_cone());
    let text = "package .\nclass Outer";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let identity = lowerer.source_identity(1).clone();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    assert!(source.header.package.is_none());
    let header = source.declarations[0].unavailable_header();
    let access = lowerer.top_level_access(header.visibility, header.name.span, "declaration", 1);
    let mut surface = CurrentUnitImports::default();
    surface.insert_unknown_package(CurrentUnitBinding {
        target: CurrentUnitTarget::Unavailable(header.kind),
        source: identity.clone(),
        file: 1,
        span: header.name.span,
        access: access.lookup,
        name: header.name.text,
    });
    let mut consumer = file(Vec::new());
    consumer.imports = vec![
        exact(&["dependency", "api", "Outer"], Some("Selected"), false),
        star(&["dependency", "api"], false),
    ];
    let resolved = surface.resolve_file_with_world(&mut lowerer, &consumer.header(), Some(&world));
    assert!(lowerer.diagnostics.is_empty());
    for targets in [
        &resolved.exact[0].targets,
        &resolved.stars[0].snapshot["Outer"],
    ] {
        assert_eq!(targets.len(), 2);
        assert!(
            targets
                .iter()
                .any(|target| target.direct_binding().is_some())
        );
        let binding = targets
            .iter()
            .find_map(ImportedTargetBinding::current_binding)
            .unwrap();
        assert!(matches!(
            surface.binding(binding).target,
            CurrentUnitTarget::Unavailable(ast::DeclarationHeaderKind::Type)
        ));
        assert_eq!(surface.binding(binding).source, identity);
    }
    surface.files.insert(0, resolved);
    lowerer.imports = surface.into();
    assert!(matches!(
        lowerer.lookup_type("Selected"),
        crate::imports::lookup::LookupResult::Unavailable(_)
    ));
    assert!(matches!(
        lowerer.lookup_type("Outer"),
        crate::imports::lookup::LookupResult::Unavailable(_)
    ));
}
