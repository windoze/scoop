use super::dependency_fixture::DependencyWorldFixture;
use super::*;

#[test]
fn reexport_plan_merges_exact_star_and_repeated_routes_by_typed_binding_key() {
    let (lowerer, _, _) = lowerer();
    let fixture =
        DependencyWorldFixture::with_nested_type(&["dependency", "api"], "Outer", "Nested");
    let world = fixture.world(lowerer.current_cone());
    let imports = vec![
        exact(&["dependency", "api", "Outer"], None, true),
        star(&["dependency", "api"], true),
        exact(&["dependency", "api", "Outer"], None, true),
        exact(&["dependency", "api", "Outer"], Some("Facade"), true),
    ];

    let first = freeze(&lowerer, &world, imports.clone());
    let second = freeze(&lowerer, &world, imports.into_iter().rev().collect());

    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    for binding in &first {
        let key = binding.identity.key();
        assert_eq!(key.exporter(), lowerer.current_cone());
        assert!(key.package().segments().is_empty());
        assert_eq!(key.target(), binding.target.persistent());
        assert_eq!(binding.routes.routes().len(), 1);
        assert_eq!(
            binding.routes.routes()[0].immediate_provider(),
            fixture.direct_identity()
        );
        assert_eq!(binding.origins.len(), 1);
    }
    let names = first
        .iter()
        .map(|binding| binding.identity.key().name().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names, ["Facade", "Outer"].into_iter().collect());
}

#[test]
fn reexport_destination_is_the_importing_source_package() {
    let (mut lowerer, _, _) = lowerer();
    lowerer.current_file = 1;
    let fixture = DependencyWorldFixture::with_nested_type(&["dependency"], "Remote", "Nested");
    let world = fixture.world(lowerer.current_cone());
    let mut source = package(file(Vec::new()), &["api"]);
    source.imports = vec![exact(&["dependency", "Remote"], Some("Facade"), true)];
    let mut surface = CurrentUnitImports::default();
    surface.files.push(FrozenFileImports::default());
    let resolved = surface.resolve_file_with_world(&mut lowerer, &source, Some(&world));
    surface.files.push(resolved);

    surface.freeze_reexports(&lowerer).unwrap();

    assert!(lowerer.diagnostics.is_empty());
    assert_eq!(surface.reexports.len(), 1);
    let key = surface.reexports[0].identity.key();
    assert_eq!(key.name().as_str(), "Facade");
    assert!(
        key.package()
            .segments()
            .iter()
            .map(|segment| segment.as_str())
            .eq(["api"])
    );
}

#[test]
fn public_star_destination_conflict_aborts_the_whole_surface() {
    let (mut lowerer, _, _) = lowerer();
    let fixture = DependencyWorldFixture::with_conflicting_types(&["dependency", "api"], "Shared");
    let world = fixture.world(lowerer.current_cone());
    let mut surface = CurrentUnitImports::default();
    let mut source = file(Vec::new());
    source.imports.push(star(&["dependency", "api"], true));
    let resolved = surface.resolve_file_with_world(&mut lowerer, &source, Some(&world));
    assert_eq!(resolved.stars.len(), 1);
    surface.files.push(resolved);

    let error = surface.freeze_reexports(&lowerer).unwrap_err();

    assert!(lowerer.diagnostics.is_empty());
    assert!(surface.reexports.is_empty());
    assert!(matches!(
        &error,
        ReexportPlanBuildError::DestinationConflict { .. }
    ));
    assert_eq!(
        error.to_string(),
        "re-export destination conflicts with another public binding"
    );
}

#[test]
fn public_star_preserves_distinct_dependency_overloads() {
    let (lowerer, _, _) = lowerer();
    let fixture =
        DependencyWorldFixture::with_function_overloads(&["dependency", "api"], "invoke", [0, 1]);
    let world = fixture.world(lowerer.current_cone());

    let reexports = freeze(&lowerer, &world, vec![star(&["dependency", "api"], true)]);

    assert_eq!(reexports.len(), 2);
    assert!(
        reexports
            .iter()
            .all(|binding| binding.identity.key().name().as_str() == "invoke")
    );
    assert_eq!(
        reexports
            .iter()
            .map(|binding| &binding.conflict)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
}

#[test]
fn public_star_rejects_same_signature_dependency_overloads() {
    let (mut lowerer, _, _) = lowerer();
    let fixture =
        DependencyWorldFixture::with_function_overloads(&["dependency", "api"], "invoke", [0, 0]);
    let world = fixture.world(lowerer.current_cone());
    let mut surface = CurrentUnitImports::default();
    let mut source = file(Vec::new());
    source.imports.push(star(&["dependency", "api"], true));
    let resolved = surface.resolve_file_with_world(&mut lowerer, &source, Some(&world));
    assert_eq!(resolved.stars.len(), 1);
    surface.files.push(resolved);

    let error = surface.freeze_reexports(&lowerer).unwrap_err();

    assert!(surface.reexports.is_empty());
    assert!(matches!(
        error,
        ReexportPlanBuildError::DestinationConflict { .. }
    ));
}

fn freeze(
    lowerer: &Lowerer,
    world: &hir::ImportedSemanticWorld<'_>,
    imports: Vec<ast::ImportSyntax>,
) -> Vec<FrozenReexportBinding> {
    let mut source = file(Vec::new());
    source.imports = imports;
    let mut resolving = lowerer.clone();
    let mut surface = CurrentUnitImports::default();
    let resolved = surface.resolve_file_with_world(&mut resolving, &source, Some(world));
    assert!(resolving.diagnostics.is_empty());
    surface.files.push(resolved);
    surface.freeze_reexports(&resolving).unwrap();
    surface.reexports
}
