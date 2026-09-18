use super::*;

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
    assert!(resolved.stars.is_empty());
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
        let expected_span = selector_span(&import);
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
        assert_eq!(lowerer.diagnostics[0].span, Some(expected_span));
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
    let mut star_source = file(Vec::new());
    star_source.imports = vec![star(&["api"], false)];
    let resolved = surface.resolve_file(&mut lowerer, &star_source);
    assert_eq!(
        resolved.stars[0].namespace,
        ResolvedImportNamespace::Current(ResolvedNamespace::Package(api))
    );
    assert!(resolved.stars[0].snapshot.is_empty());
    assert!(lowerer.diagnostics.is_empty());

    let mut exact_source = file(Vec::new());
    exact_source.imports = vec![exact(&["api"], None, false)];
    let resolved = surface.resolve_file(&mut lowerer, &exact_source);
    assert!(resolved.exact.is_empty() && resolved.stars.is_empty());
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
    let mut valid_source = file(Vec::new());
    valid_source.imports = vec![
        exact(&["api", "Host", "factory"], None, false),
        star(&["api", "Host"], false),
    ];
    let resolved = surface.resolve_file(&mut lowerer, &valid_source);
    assert_eq!(resolved.exact.len(), 1);
    assert_eq!(
        resolved.exact[0].targets.first().current_binding(),
        Some(member)
    );
    assert_eq!(
        resolved.stars[0].snapshot["factory"]
            .first()
            .current_binding(),
        Some(member)
    );
    assert!(lowerer.diagnostics.is_empty());

    let mut shadowed_source = file(Vec::new());
    shadowed_source.imports = vec![exact(&["api", "child", "factory"], None, false)];
    let resolved = surface.resolve_file(&mut lowerer, &shadowed_source);
    assert!(resolved.exact.is_empty() && resolved.stars.is_empty());
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
