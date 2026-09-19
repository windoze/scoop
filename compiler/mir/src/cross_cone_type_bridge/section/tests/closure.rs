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
    consumer.source.uses = vec![provider.shape_use()];
    let graph = graph(&[&provider, &consumer]);
    let provider_section = provider.section(&[], &graph).unwrap();
    let consumer_section = consumer.section(&[&provider_section], &graph).unwrap();
    assert_eq!(consumer_section.selected().len(), 5);
    let reference = consumer_section
        .selected()
        .reference(provider.source.provider, provider.type_use().target())
        .unwrap();
    assert!(
        matches!(consumer_section.selected().resolve(reference), Some(MirTypeBridgeSemanticRecordV1::Type(record)) if record.exact() == provider.types.payload.id())
    );
    let again = consumer.section(&[&provider_section], &graph).unwrap();
    assert!(again.selected().resolve(reference).is_none());
    assert!(again.selected().relation(reference).is_none());
}

#[test]
fn local_field_closes_foreign_type_without_requesting_its_shape_family() {
    let provider = Fixture::new("field-provider");
    let mut consumer = Fixture::new("field-consumer");
    let graph = graph(&[&provider, &consumer]);
    consumer.with_field(provider.types.payload.id(), &graph);
    let provider_section = provider.section(&[], &graph).unwrap();
    let consumer_section = consumer.section(&[&provider_section], &graph).unwrap();
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
    facade.source.uses = vec![provider.type_use()];
    consumer.source.uses = facade.source.uses.clone();
    let graph = graph(&[&provider, &facade, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let middle = facade.section(&[&terminal], &graph).unwrap();
    let client = consumer.section(&[&middle], &graph).unwrap();
    assert_eq!(
        client.selected().relations().collect::<Vec<_>>(),
        vec![provider.type_use()]
    );
    consumer.source.uses = vec![MirTypeBridgeDependencyV1::new(
        facade.source.provider,
        provider.type_use().target(),
    )];
    assert!(matches!(
        consumer.section(&[&middle], &graph),
        Err(MirTypeBridgeSectionError::MissingTarget(_))
    ));
}

#[test]
fn diamond_accepts_the_same_terminal_instance_but_rejects_duplicate_authority() {
    let provider = Fixture::new("diamond-root");
    let left = Fixture::new("left");
    let right = Fixture::new("right");
    let mut consumer = Fixture::new("diamond-client");
    consumer.source.uses = vec![provider.type_use()];
    let graph = graph(&[&provider, &left, &right, &consumer]);
    let first = provider.section(&[], &graph).unwrap();
    let second = provider.section(&[], &graph).unwrap();
    let left_section = left.section(&[&first], &graph).unwrap();
    let right_section = right.section(&[&first], &graph).unwrap();
    assert_eq!(
        consumer
            .section(&[&left_section, &right_section], &graph)
            .unwrap()
            .selected()
            .len(),
        1
    );
    assert!(matches!(
        consumer.section(&[&first, &first], &graph),
        Err(MirTypeBridgeSectionError::DuplicateProvider(_))
    ));
    let other_right = right.section(&[&second], &graph).unwrap();
    assert!(matches!(
        consumer.section(&[&left_section, &other_right], &graph),
        Err(MirTypeBridgeSectionError::DuplicateProvider(_))
    ));
}

#[test]
fn independent_source_inventory_and_committed_use_order_are_mandatory() {
    let mut fixture = Fixture::new("source-contract");
    fixture.source.types.pop();
    assert!(matches!(
        fixture.section(&[], &fixture.types.graph),
        Err(MirTypeBridgeSectionError::SourceJoin(_))
    ));
    let provider = Fixture::new("used");
    let mut consumer = Fixture::new("uses");
    consumer.source.uses = vec![provider.type_use(), provider.type_use()];
    let graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    assert!(matches!(
        consumer.section(&[&terminal], &graph),
        Err(MirTypeBridgeSectionError::NonCanonicalCommittedUses)
    ));
    consumer.source.uses = vec![consumer.type_use()];
    assert!(matches!(
        consumer.section(&[&terminal], &graph),
        Err(MirTypeBridgeSectionError::SelectedCurrentProvider)
    ));
}
