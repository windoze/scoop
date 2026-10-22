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
        &[header],
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

    let empty = CurrentUnitImports::default();
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
