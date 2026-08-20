use scoop_identity::{BindingNamespace, BindingRole, ConeCoordinate, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, coordinate, empty_alias_expansions, identifiers, import_foundation, package,
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
    let world = ImportedSemanticWorld::from_dependencies(
        coordinate("declared-current").identity().unwrap(),
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
        Vec::new(),
    )
    .unwrap();

    let group = world
        .resolve_direct_exact(&identifiers(&["demo", "Visible"]), BindingNamespace::Type)
        .unwrap();
    let target = group.targets().next().unwrap();
    let source = target.sources().next().unwrap();
    let route = source.route();
    assert_eq!(group.len(), 1);
    assert_eq!(group.binding_count(), 1);
    assert_eq!(target.source_count(), 1);
    assert_eq!(target.binding_target().namespace(), BindingNamespace::Type);
    assert_eq!(target.binding_target().role(), BindingRole::TypeName);
    assert_eq!(source.route().immediate_provider(), direct.identity());
    assert_eq!(
        source.route().hops()[0].binding(),
        direct.outer_binding.unwrap()
    );
    let mut merged = target.clone();
    merged.try_merge(target.clone()).unwrap();
    assert_eq!(merged.source_count(), 1);
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
    let world = ImportedSemanticWorld::from_dependencies(
        coordinate("diamond-current").identity().unwrap(),
        vec![
            ImportedProviderInput {
                foundation: &core_foundation,
                interface: &core.interface,
                alias_expansions: &aliases,
            },
            ImportedProviderInput {
                foundation: &second_foundation,
                interface: &second.interface,
                alias_expansions: &aliases,
            },
            ImportedProviderInput {
                foundation: &first_foundation,
                interface: &first.interface,
                alias_expansions: &aliases,
            },
        ],
        vec![ImportedProviderInput {
            foundation: &terminal_foundation,
            interface: &terminal.interface,
            alias_expansions: &aliases,
        }],
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
    let mut expected = [first.identity(), second.identity()];
    expected.sort_unstable();
    assert_eq!(
        sources
            .iter()
            .map(|source| source.route().immediate_provider())
            .collect::<Vec<_>>(),
        expected
    );
    let excluded = world.direct_provider(first.identity()).unwrap().id();
    let filtered = world
        .direct_package(&package(&["public"]))
        .unwrap()
        .snapshot_filtered(|provider| provider != excluded);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].targets().len(), 1);
    let retained = &filtered[0].targets()[0];
    assert_eq!(retained.target(), target.target());
    assert_eq!(retained.source_count(), 1);
    assert_eq!(
        retained
            .sources()
            .next()
            .unwrap()
            .route()
            .immediate_provider(),
        second.identity()
    );
    for source in sources {
        let route = source.route();
        assert_eq!(route.hops().len(), 2);
        assert_eq!(
            route.hops()[0].exporter(),
            source.route().immediate_provider()
        );
        let source_binding = if route.immediate_provider() == first.identity() {
            first.outer_binding.unwrap()
        } else {
            second.outer_binding.unwrap()
        };
        assert_eq!(route.hops()[0].binding(), source_binding);
        assert_eq!(route.terminal().exporter(), terminal.identity());
        assert_eq!(route.terminal().binding(), terminal.outer_binding.unwrap());
    }
}
