use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, coordinate, empty_alias_expansions, import_foundation, package,
};

#[test]
fn empty_semantic_world_uses_the_same_contract_for_every_current_cone() {
    let ordinary = coordinate("current").identity().unwrap();
    for current in [ConeIdentity::CORE, ordinary] {
        let world =
            ImportedSemanticWorld::from_dependencies(current, Vec::new(), Vec::new()).unwrap();
        assert_eq!(world.current(), current);
        assert_eq!(world.provider_count(), 0);
        assert_eq!(world.direct_provider_count(), 0);
        assert_eq!(world.support_provider_count(), 0);
        assert!(world.direct_provider(ConeIdentity::CORE).is_none());
        assert_eq!(world.direct_packages().package_count(), 0);
    }
}

#[test]
fn every_provider_uses_the_same_current_cone_exclusion() {
    for coordinate in [ConeCoordinate::reserved_core(), coordinate("current")] {
        let fixture = ProviderFixture::empty(coordinate);
        let mut session = SemanticIdentitySession::new();
        let foundation = import_foundation(&mut session, &fixture, 1);
        let aliases = empty_alias_expansions();
        let current = fixture.identity();
        let result = ImportedSemanticWorld::from_dependencies(
            current,
            vec![ImportedProviderInput {
                foundation: &foundation,
                interface: &fixture.interface,
                alias_expansions: &aliases,
            }],
            Vec::new(),
        );
        assert!(
            matches!(result, Err(ImportedSemanticWorldBuildError::CurrentUsedAsProvider(id)) if id == current)
        );
    }
}

#[test]
fn direct_dependencies_control_package_lookup_and_all_dependencies_resolve_declarations() {
    let core = ProviderFixture::with_nominals(
        ConeCoordinate::reserved_core(),
        package(&["core", "extensions"]),
        "CoreType",
        None,
    );
    let direct = ProviderFixture::with_nominals(
        coordinate("direct"),
        package(&["demo", "api"]),
        "Outer",
        Some("Nested"),
    );
    let support = ProviderFixture::with_nominals(
        coordinate("support"),
        package(&["hidden"]),
        "SupportType",
        None,
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 1);
    let direct_foundation = import_foundation(&mut session, &direct, 2);
    let support_foundation = import_foundation(&mut session, &support, 3);
    let aliases = empty_alias_expansions();
    let current = coordinate("current").identity().unwrap();

    let world = ImportedSemanticWorld::from_dependencies(
        current,
        vec![
            ImportedProviderInput {
                foundation: &core_foundation,
                interface: &core.interface,
                alias_expansions: &aliases,
            },
            ImportedProviderInput {
                foundation: &direct_foundation,
                interface: &direct.interface,
                alias_expansions: &aliases,
            },
        ],
        vec![ImportedProviderInput {
            foundation: &support_foundation,
            interface: &support.interface,
            alias_expansions: &aliases,
        }],
    )
    .unwrap();

    assert_eq!(world.provider_count(), 3);
    assert_eq!(world.direct_provider_count(), 2);
    assert_eq!(world.support_provider_count(), 1);
    assert_eq!(
        world.direct_provider(ConeIdentity::CORE).unwrap().id(),
        ConeIdentity::CORE
    );

    let direct_view = world.direct_provider(direct.identity()).unwrap();
    assert_eq!(direct_view.public_bindings().len(), 2);
    assert_eq!(
        direct_view.nominal_interfaces(),
        direct.interface.nominal_interfaces()
    );
    assert_eq!(
        world
            .direct_provider(ConeIdentity::CORE)
            .unwrap()
            .nominal_interfaces(),
        core.interface.nominal_interfaces(),
    );
    assert!(world.nominal(support.outer.unwrap()).is_some());
    assert!(world.direct_provider(support.identity()).is_none());

    assert!(world.direct_packages().contains(&package(&["demo", "api"])));
    assert!(!world.direct_packages().contains(&package(&["hidden"])));
    assert_eq!(world.direct_packages().package_count(), 2);
    let core_package = world
        .direct_package(&package(&["core", "extensions"]))
        .unwrap();
    let binding = core_package
        .binding_group(scoop_identity::BindingNamespace::Type, "CoreType")
        .unwrap();
    assert_eq!(binding.len(), 1);
    assert!(
        matches!(core.outer, Some(crate::SourceNominalId::Concrete(id))
        if binding.bindings().next().unwrap().target().persistent()
            == scoop_identity::BindableEntity::Type(id))
    );

    for fixture in [&core, &direct, &support] {
        let crate::SourceNominalId::Concrete(source) = fixture.outer.unwrap() else {
            unreachable!("these fixtures contain concrete declarations")
        };

        assert!(
            world
                .has_materializable_nominal_source(fixture.identity(), source)
                .unwrap()
        );
        let other = if fixture.identity() == core.identity() {
            direct.identity()
        } else {
            core.identity()
        };
        for wrong in [current, other] {
            assert!(matches!(
                world.has_materializable_nominal_source(wrong, source),
                Err(crate::NominalMaterializationClosureError::MissingNominal(actual)) if actual == source
            ));
        }
    }
}
