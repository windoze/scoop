use super::*;

#[test]
fn complete_local_family_closes_without_importing_any_target() {
    let fixture = Fixture::new("local");
    let section = fixture.section(&[], &fixture.types.graph).unwrap();
    assert_eq!(section.types().records().len(), 4);
    assert_eq!(section.shape_support().records().len(), 1);
    assert!(section.selected().is_empty());
    assert!(section.initialization_units().is_empty());
}

#[test]
fn source_shape_request_closes_all_helpers_and_resolves_terminal_records() {
    let provider = Fixture::new("provider");
    let mut consumer = Fixture::new("consumer");
    consumer.uses = vec![provider.shape_use()];
    let graph = graph(&[&provider, &consumer]);
    let provider_section = provider.section(&[], &graph).unwrap();
    let consumer_section = consumer
        .section(&[provider_section.dependency_view()], &graph)
        .unwrap();
    assert_eq!(consumer_section.selected().len(), 5);
    assert!(
        matches!(consumer_section.selected().record(provider.provider, provider.type_use().target()), Some(MirTypeBridgeSemanticRecordV1::Type(record)) if record.exact() == provider.types.payload.id())
    );
}

#[test]
fn local_field_closes_foreign_type_without_requesting_its_shape_family() {
    let provider = Fixture::new("field-provider");
    let mut consumer = Fixture::new("field-consumer");
    let graph = graph(&[&provider, &consumer]);
    consumer.with_field(provider.types.payload.id(), &graph);
    let provider_section = provider.section(&[], &graph).unwrap();
    let consumer_section = consumer
        .section(&[provider_section.dependency_view()], &graph)
        .unwrap();
    assert_eq!(
        consumer_section.selected().relations().collect::<Vec<_>>(),
        vec![provider.type_use()]
    );
    assert!(matches!(
        consumer.section(&[], &graph),
        Err(MirTypeBridgeSectionError::MissingDependency(_))
    ));
}

#[test]
fn facade_selection_preserves_the_terminal_provider() {
    let provider = Fixture::new("terminal");
    let mut facade = Fixture::new("facade");
    let mut consumer = Fixture::new("client");
    facade.uses = vec![provider.type_use()];
    consumer.uses = facade.uses.clone();
    let graph = graph(&[&provider, &facade, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let middle = facade
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let client = consumer
        .section(
            &[middle.dependency_view(), terminal.dependency_view()],
            &graph,
        )
        .unwrap();
    assert_eq!(
        client.selected().relations().collect::<Vec<_>>(),
        vec![provider.type_use()]
    );
    consumer.uses = vec![MirTypeBridgeDependencyV1::new(
        facade.provider,
        provider.type_use().target(),
    )];
    assert!(matches!(
        consumer.section(
            &[middle.dependency_view(), terminal.dependency_view()],
            &graph
        ),
        Err(MirTypeBridgeSectionError::MissingTarget(_))
    ));
}

#[test]
fn dependency_catalog_accepts_unique_providers_and_rejects_duplicate_entries() {
    let provider = Fixture::new("diamond-root");
    let left = Fixture::new("left");
    let right = Fixture::new("right");
    let mut consumer = Fixture::new("diamond-client");
    consumer.uses = vec![provider.type_use()];
    let graph = graph(&[&provider, &left, &right, &consumer]);
    let first = provider.section(&[], &graph).unwrap();
    let second = provider.section(&[], &graph).unwrap();
    let left_section = left.section(&[first.dependency_view()], &graph).unwrap();
    let right_section = right.section(&[first.dependency_view()], &graph).unwrap();
    assert_eq!(
        consumer
            .section(
                &[
                    left_section.dependency_view(),
                    right_section.dependency_view(),
                    first.dependency_view()
                ],
                &graph
            )
            .unwrap()
            .selected()
            .len(),
        1
    );
    assert!(matches!(
        consumer.section(&[first.dependency_view(), first.dependency_view()], &graph),
        Err(MirTypeBridgeSectionError::DuplicateProvider(_))
    ));
    assert!(matches!(
        consumer.section(&[first.dependency_view(), second.dependency_view()], &graph),
        Err(MirTypeBridgeSectionError::DuplicateProvider(_))
    ));
}

#[test]
fn committed_uses_must_be_canonical_and_external() {
    let provider = Fixture::new("used");
    let mut consumer = Fixture::new("uses");
    consumer.uses = vec![provider.type_use(), provider.type_use()];
    let graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    assert!(matches!(
        consumer.section(&[terminal.dependency_view()], &graph),
        Err(MirTypeBridgeSectionError::NonCanonicalCommittedUses)
    ));
    consumer.uses = vec![consumer.type_use()];
    assert!(matches!(
        consumer.section(&[terminal.dependency_view()], &graph),
        Err(MirTypeBridgeSectionError::SelectedCurrentProvider)
    ));
}
