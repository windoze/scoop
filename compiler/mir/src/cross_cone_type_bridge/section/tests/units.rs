use super::identity_support::{add_units, set_uses};
use super::*;

#[test]
fn selected_initialization_unit_retains_both_callable_definitions() {
    let core_fixture = core::fixture();
    let mut provider = Fixture::new("unit-provider");
    add_units(&mut provider, &core_fixture, &["setting"]);
    let mut consumer = Fixture::new("unit-consumer");
    let unit = provider.units[0].unit();
    let relation = MirTypeBridgeDependencyV1::new(
        provider.provider,
        MirTypeBridgeTargetV1::InitializationUnit(unit),
    );
    consumer.uses = vec![relation];
    let graph = graph(&[&core_fixture, &provider, &consumer]);
    let core = core_fixture.section(&[], &graph).unwrap();
    let terminal = provider.section(&[core.dependency_view()], &graph).unwrap();
    let contract = &terminal.initialization_units()[0];
    assert_ne!(contract.initializer(), contract.ensure());
    let section = consumer
        .section(
            &[terminal.dependency_view(), core.dependency_view()],
            &graph,
        )
        .unwrap();
    let mut expected = vec![relation, core_fixture.type_use()];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
    assert!(
        matches!(section.selected().record(relation.provider(), relation.target()), Some(MirTypeBridgeSemanticRecordV1::InitializationUnit(record)) if record.unit() == unit)
    );
}

#[test]
fn selected_unit_follows_its_committed_ensure_edges_but_not_other_units_edges() {
    let core_fixture = core::fixture();
    let mut leaf = Fixture::new("unit-leaf");
    add_units(&mut leaf, &core_fixture, &["one", "two"]);
    let mut middle = Fixture::new("unit-middle");
    add_units(&mut middle, &core_fixture, &["used", "unused"]);
    let mut consumer = Fixture::new("unit-client");
    let graph = graph(&[&core_fixture, &leaf, &middle, &consumer]);
    let uses = middle
        .units
        .iter()
        .zip(&leaf.units)
        .map(|(local, dependency)| {
            SelectedExternalInitializationUseV1::try_new(
                middle.provider,
                &graph,
                local.unit(),
                leaf.provider,
                dependency.unit(),
                MirExternalInitializationCauseV1::InitializationSupport(dependency.unit()),
            )
            .unwrap()
        })
        .collect();
    set_uses(&mut middle, uses);
    let relation = MirTypeBridgeDependencyV1::new(
        middle.provider,
        MirTypeBridgeTargetV1::InitializationUnit(middle.units[0].unit()),
    );
    consumer.uses = vec![relation];
    let core = core_fixture.section(&[], &graph).unwrap();
    let leaf_section = leaf.section(&[core.dependency_view()], &graph).unwrap();
    let middle_section = middle
        .section(
            &[leaf_section.dependency_view(), core.dependency_view()],
            &graph,
        )
        .unwrap();
    let section = consumer
        .section(
            &[
                middle_section.dependency_view(),
                leaf_section.dependency_view(),
                core.dependency_view(),
            ],
            &graph,
        )
        .unwrap();
    let mut expected = vec![
        relation,
        core_fixture.type_use(),
        MirTypeBridgeDependencyV1::new(
            leaf.provider,
            MirTypeBridgeTargetV1::InitializationUnit(leaf.units[0].unit()),
        ),
    ];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
}

#[test]
fn initialization_use_cannot_supply_a_missing_local_unit_inventory() {
    let core_fixture = core::fixture();
    let mut provider = Fixture::new("use-target");
    add_units(&mut provider, &core_fixture, &["foreign"]);
    let mut consumer = Fixture::new("use-source");
    add_units(&mut consumer, &core_fixture, &["local"]);
    let graph = graph(&[&core_fixture, &provider, &consumer]);
    let use_record = SelectedExternalInitializationUseV1::try_new(
        consumer.provider,
        &graph,
        consumer.units[0].unit(),
        provider.provider,
        provider.units[0].unit(),
        MirExternalInitializationCauseV1::InitializationSupport(provider.units[0].unit()),
    )
    .unwrap();
    set_uses(&mut consumer, vec![use_record]);
    consumer.units.clear();
    let core = core_fixture.section(&[], &graph).unwrap();
    let target = provider.section(&[core.dependency_view()], &graph).unwrap();
    assert!(matches!(
        consumer.section(&[target.dependency_view(), core.dependency_view()], &graph),
        Err(MirTypeBridgeSectionError::MissingDependency(
            MirTypeBridgeTargetV1::InitializationUnit(_)
        ))
    ));
}
