use scoop_identity::{BindingNamespace, BindingRole, ConeCoordinate, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, certificate, coordinate, empty_alias_expansions, identifiers,
    import_foundation, package,
};

#[test]
fn declared_direct_binding_produces_one_public_lookup_witness() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let direct = ProviderFixture::with_nominals(
        coordinate("declared-provider"),
        package(&["demo"]),
        "Visible",
        None,
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 31);
    let direct_foundation = import_foundation(&mut session, &direct, 32);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("declared-current").identity().unwrap(),
        Some(TrustedCoreImportedProviderInput::from_validated(
            certificate(&core.coordinate, 31),
            &core_foundation,
            &core.interface,
            &aliases,
        )),
        vec![DirectImportedProviderInput::from_validated(
            certificate(&direct.coordinate, 32),
            &direct_foundation,
            &direct.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();

    let group = world
        .resolve_direct_exact(&identifiers(&["demo", "Visible"]), BindingNamespace::Type)
        .unwrap();
    let target = group.targets().next().unwrap();
    let source = target.sources().next().unwrap();
    let route = source.witness().route();
    assert_eq!(group.len(), 1);
    assert_eq!(group.binding_count(), 1);
    assert_eq!(target.source_count(), 1);
    assert_eq!(target.binding_target().namespace(), BindingNamespace::Type);
    assert_eq!(target.binding_target().role(), BindingRole::TypeName);
    assert_eq!(source.provider_identity(), direct.identity());
    assert_eq!(
        source.exported_binding().persistent(),
        direct.outer_binding.unwrap()
    );
    assert_eq!(source.witness().terminal_declaration(), target.target());
    assert_eq!(route.immediate_provider(), direct.identity());
    assert_eq!(route.hops().len(), 1);
    assert_eq!(route.terminal().binding(), direct.outer_binding.unwrap());
}

#[test]
fn diamond_reexports_fold_target_and_preserve_canonical_routes() {
    let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
    let terminal = ProviderFixture::with_nominals(
        coordinate("diamond-terminal"),
        package(&["origin"]),
        "Target",
        None,
    );
    let first = ProviderFixture::reexporting_type(
        coordinate("diamond-first"),
        package(&["public"]),
        "Shared",
        terminal.outer_key.clone().unwrap(),
        terminal.identity(),
        terminal.outer_binding.unwrap(),
    );
    let second = ProviderFixture::reexporting_type(
        coordinate("diamond-second"),
        package(&["public"]),
        "Shared",
        terminal.outer_key.clone().unwrap(),
        terminal.identity(),
        terminal.outer_binding.unwrap(),
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 33);
    let terminal_foundation = import_foundation(&mut session, &terminal, 34);
    let first_foundation = import_foundation(&mut session, &first, 35);
    let second_foundation = import_foundation(&mut session, &second, 36);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        coordinate("diamond-current").identity().unwrap(),
        Some(TrustedCoreImportedProviderInput::from_validated(
            certificate(&core.coordinate, 33),
            &core_foundation,
            &core.interface,
            &aliases,
        )),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&second.coordinate, 36),
                &second_foundation,
                &second.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&first.coordinate, 35),
                &first_foundation,
                &first.interface,
                &aliases,
            ),
        ],
        vec![SupportImportedProviderInput::from_validated(
            certificate(&terminal.coordinate, 34),
            &terminal_foundation,
            &terminal.interface,
            &aliases,
        )],
    )
    .unwrap();

    let group = world
        .resolve_direct_exact(&identifiers(&["public", "Shared"]), BindingNamespace::Type)
        .unwrap();
    let target = group.targets().next().unwrap();
    let sources = target.sources().collect::<Vec<_>>();
    assert_eq!(group.len(), 1);
    assert_eq!(group.binding_count(), 2);
    assert_eq!(sources.len(), 2);
    let SourceNominalId::Concrete(terminal_type) = terminal.outer.unwrap() else {
        panic!("fixture target must be a concrete nominal");
    };
    assert_eq!(
        target.target().persistent(),
        scoop_identity::BindableEntity::Type(terminal_type)
    );
    assert_eq!(sources[0].provider_identity(), first.identity());
    assert_eq!(sources[1].provider_identity(), second.identity());
    for source in sources {
        let route = source.witness().route();
        assert_eq!(route.immediate_provider(), source.provider_identity());
        assert_eq!(route.hops().len(), 2);
        assert_eq!(route.hops()[0].exporter(), source.provider_identity());
        assert_eq!(
            route.hops()[0].binding(),
            source.exported_binding().persistent()
        );
        assert_eq!(route.terminal().exporter(), terminal.identity());
        assert_eq!(route.terminal().binding(), terminal.outer_binding.unwrap());
    }
}
