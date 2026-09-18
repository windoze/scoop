use scoop_identity::{BindingNamespace, ConeCoordinate, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, certificate, coordinate, empty_alias_expansions, identifiers,
    import_foundation, package,
};

#[test]
fn package_lookup_uses_static_edges_and_excludes_nested_bindings_from_package_scope() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let direct = ProviderFixture::with_nominals(
        coordinate("static-owner"),
        package(&["demo", "api"]),
        "Outer",
        Some("Nested"),
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 4);
    let direct_foundation = import_foundation(&mut session, &direct, 5);
    let aliases = empty_alias_expansions();
    let current = coordinate("static-current").identity().unwrap();
    let world = ImportedSemanticWorld::from_validated_closure(
        current,
        Some(TrustedCoreImportedProviderInput::from_validated(
            certificate(&core.coordinate, 4),
            &core_foundation,
            &core.interface,
            &aliases,
        )),
        vec![DirectImportedProviderInput::from_validated(
            certificate(&direct.coordinate, 5),
            &direct_foundation,
            &direct.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();

    let package_view = world.direct_package(&package(&["demo", "api"])).unwrap();
    assert_eq!(package_view.contribution_count(), 1);
    assert_eq!(package_view.bindings().count(), 1);
    let snapshot = package_view.snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].namespace(), BindingNamespace::Type);
    assert_eq!(snapshot[0].name().as_str(), "Outer");
    assert_eq!(snapshot[0].targets().len(), 1);
    assert!(
        package_view
            .binding_group(BindingNamespace::Type, "Nested")
            .is_none()
    );

    let path = identifiers(&["demo", "api", "Outer", "Nested"]);
    let group = world
        .resolve_direct_exact(&path, BindingNamespace::Type)
        .unwrap();
    assert_eq!(group.len(), 1);
    assert_eq!(
        group.bindings().next().unwrap().target().persistent(),
        scoop_identity::BindableEntity::Type(direct.nested.unwrap())
    );

    let owner_path = identifiers(&["demo", "api", "Outer"]);
    let DirectNamespaceView::Static(namespace) =
        world.resolve_direct_namespace(&owner_path).unwrap()
    else {
        panic!("the qualified outer type must resolve to a static namespace");
    };
    assert_eq!(
        namespace.owner().declaration().persistent(),
        direct.outer.unwrap()
    );
    assert_eq!(namespace.bindings().count(), 1);
}

#[test]
fn longest_package_prefix_never_falls_back_to_a_shorter_static_path() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let outer = ProviderFixture::with_nominals(
        coordinate("outer-provider"),
        package(&["demo", "api"]),
        "Outer",
        Some("Nested"),
    );
    let shadow = ProviderFixture::with_nominals(
        coordinate("package-provider"),
        package(&["demo", "api", "Outer"]),
        "Leaf",
        None,
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 6);
    let outer_foundation = import_foundation(&mut session, &outer, 7);
    let shadow_foundation = import_foundation(&mut session, &shadow, 8);
    let aliases = empty_alias_expansions();
    let current = coordinate("prefix-current").identity().unwrap();
    let world = ImportedSemanticWorld::from_validated_closure(
        current,
        Some(TrustedCoreImportedProviderInput::from_validated(
            certificate(&core.coordinate, 6),
            &core_foundation,
            &core.interface,
            &aliases,
        )),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&shadow.coordinate, 8),
                &shadow_foundation,
                &shadow.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&outer.coordinate, 7),
                &outer_foundation,
                &outer.interface,
                &aliases,
            ),
        ],
        Vec::new(),
    )
    .unwrap();

    let path = identifiers(&["demo", "api", "Outer", "Nested"]);
    assert_eq!(
        world
            .direct_packages()
            .longest_prefix(&path)
            .unwrap()
            .consumed_segments(),
        3
    );
    assert!(matches!(
        world.resolve_direct_exact(&path, BindingNamespace::Type),
        Err(DirectNamespaceLookupError::MissingBinding { segment: 3, .. })
    ));
    let direct_order = world
        .direct_providers()
        .map(|provider| provider.certificate().identity())
        .collect::<Vec<_>>();
    assert!(direct_order.windows(2).all(|pair| pair[0] < pair[1]));
}
