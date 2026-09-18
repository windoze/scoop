use scoop_identity::BindingNamespace;

use super::dependency_fixture::DependencyWorldFixture;
use super::*;

#[test]
fn direct_exact_star_and_static_selectors_preserve_typed_witnesses() {
    let (mut lowerer, _, _) = lowerer();
    let fixture =
        DependencyWorldFixture::with_nested_type(&["dependency", "api"], "Outer", "Nested");
    let world = fixture.world(lowerer.current_cone());
    let mut source = file(Vec::new());
    source.imports = vec![
        exact(&["dependency", "api", "Outer"], Some("Facade"), false),
        exact(&["dependency", "api", "Outer"], Some("Facade"), false),
        exact(&["dependency", "api", "Outer", "Nested"], None, false),
        star(&["dependency", "api"], false),
        star(&["dependency", "api", "Outer"], false),
    ];

    let mut surface = CurrentUnitImports::default();
    let resolved = surface.resolve_file_with_world(&mut lowerer, &source, Some(&world));

    assert!(lowerer.diagnostics.is_empty());
    assert_eq!(resolved.exact.len(), 3);
    assert_eq!(resolved.stars.len(), 2);
    let outer = resolved.exact[0]
        .targets
        .first()
        .direct_binding()
        .expect("the exact target comes from a direct dependency");
    assert_eq!(outer.binding_target().namespace(), BindingNamespace::Type);
    assert_eq!(outer.source_count(), 1);
    assert_eq!(
        outer.sources().next().unwrap().provider_identity(),
        fixture.direct_identity()
    );
    assert!(matches!(
        &resolved.stars[0].namespace,
        ResolvedImportNamespace::DirectPackage(path)
            if path.segments().iter().map(|segment| segment.as_str()).eq(["dependency", "api"])
    ));
    assert!(matches!(
        resolved.stars[1].namespace,
        ResolvedImportNamespace::DirectStatic(_)
    ));
    assert!(resolved.stars[0].snapshot.contains_key("Outer"));
    assert!(resolved.stars[1].snapshot.contains_key("Nested"));

    surface.files.push(resolved);
    let exact_layer = surface.layers(
        0,
        crate::namespace::TopLevelNamespaces::root_package(),
        "Facade",
    );
    assert!(exact_layer[0].bindings.is_empty());
    assert_eq!(exact_layer[0].dependency_bindings.len(), 1);
    assert_eq!(exact_layer[0].dependency_bindings[0].source_count(), 1);
    let star_layer = surface.layers(
        0,
        crate::namespace::TopLevelNamespaces::root_package(),
        "Outer",
    );
    assert!(star_layer[2].bindings.is_empty());
    assert_eq!(star_layer[2].dependency_bindings.len(), 1);
}

#[test]
fn equal_package_prefix_merges_current_and_direct_star_snapshots() {
    let (mut lowerer, api, _) = lowerer();
    let fixture = DependencyWorldFixture::with_nested_type(&["api"], "Remote", "Nested");
    let world = fixture.world(lowerer.current_cone());
    let mut surface = CurrentUnitImports::default();
    let local = function(
        &mut surface,
        &lowerer,
        ResolvedNamespace::Package(api),
        "local",
        1,
        91,
        false,
    );
    let mut source = file(Vec::new());
    source.imports.push(star(&["api"], false));

    let resolved = surface.resolve_file_with_world(&mut lowerer, &source, Some(&world));

    assert!(lowerer.diagnostics.is_empty());
    assert!(matches!(
        &resolved.stars[0].namespace,
        ResolvedImportNamespace::SplitPackage { current, direct }
            if *current == api && direct.segments().iter().map(|segment| segment.as_str()).eq(["api"])
    ));
    assert_eq!(
        resolved.stars[0].snapshot["local"]
            .first()
            .current_binding(),
        Some(local)
    );
    assert!(
        resolved.stars[0].snapshot["Remote"]
            .first()
            .direct_binding()
            .is_some()
    );
}

#[test]
fn globally_longest_current_package_never_falls_back_to_direct_static_owner() {
    let (mut lowerer, _, _) = lowerer();
    let fixture = DependencyWorldFixture::with_nested_type(&["api"], "child", "Nested");
    let world = fixture.world(lowerer.current_cone());
    let mut source = file(Vec::new());
    source
        .imports
        .push(exact(&["api", "child", "Nested"], None, false));

    let resolved =
        CurrentUnitImports::default().resolve_file_with_world(&mut lowerer, &source, Some(&world));

    assert!(resolved.exact.is_empty());
    assert_eq!(lowerer.diagnostics.len(), 1);
    assert_eq!(
        lowerer.diagnostics[0].message,
        "import target is not available in the current compilation unit"
    );
}
