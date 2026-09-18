use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, certificate, coordinate, empty_alias_expansions, import_foundation, package,
};

#[test]
fn core_world_is_the_only_world_without_a_trusted_core_provider() {
    let world = ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        None,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(world.current(), ConeIdentity::CORE);
    assert_eq!(world.provider_count(), 0);
    assert_eq!(world.direct_provider_count(), 0);
    assert_eq!(world.support_provider_count(), 0);
    assert!(world.trusted_core().is_none());
    assert_eq!(world.direct_packages().package_count(), 0);

    let current = ConeCoordinate::new("test", "current", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    assert!(matches!(
        ImportedSemanticWorld::from_validated_closure(current, None, Vec::new(), Vec::new(),),
        Err(ImportedSemanticWorldBuildError::MissingTrustedCore)
    ));
}

#[test]
fn world_separates_direct_enumeration_from_support_exact_lookup() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
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

    let world = ImportedSemanticWorld::from_validated_closure(
        current,
        Some(TrustedCoreImportedProviderInput::from_validated(
            certificate(&core.coordinate, 1),
            &core_foundation,
            &core.interface,
            &aliases,
        )),
        vec![DirectImportedProviderInput::from_validated(
            certificate(&direct.coordinate, 2),
            &direct_foundation,
            &direct.interface,
            &aliases,
        )],
        vec![SupportImportedProviderInput::from_validated(
            certificate(&support.coordinate, 3),
            &support_foundation,
            &support.interface,
            &aliases,
        )],
    )
    .unwrap();

    assert_eq!(world.provider_count(), 3);
    assert_eq!(world.direct_provider_count(), 2);
    assert_eq!(world.support_provider_count(), 1);
    assert_eq!(
        world.trusted_core().unwrap().certificate().identity(),
        ConeIdentity::CORE
    );

    let direct_view = world.direct_provider(direct.identity()).unwrap();
    assert_eq!(direct_view.public_bindings().len(), 2);
    assert!(world.support_provider(direct.identity()).is_none());
    let support_view = world.support_provider(support.identity()).unwrap();
    assert!(
        support_view
            .typed()
            .nominal(support.outer.unwrap())
            .is_some()
    );
    assert!(world.direct_provider(support.identity()).is_none());

    assert!(world.direct_packages().contains(&package(&["demo", "api"])));
    assert!(!world.direct_packages().contains(&package(&["hidden"])));
    assert_eq!(world.direct_packages().package_count(), 1);
}
