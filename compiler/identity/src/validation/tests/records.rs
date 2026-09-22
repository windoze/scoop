use super::*;

#[test]
fn closure_records_share_keys_without_redeclaring_dependency_records() {
    let external = validated_source_type_graph();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(&external)
        .unwrap();
    pending
        .register_external_graph_authorities(&external)
        .unwrap();
    let graph = pending.finish().unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let records = graph
        .closure_records::<PersistentTypeId, SourceDeclarationKey>(&mut meter, &WirePath::root())
        .unwrap();
    assert_eq!(records.len(), 1);
    let expected = source_type_record();
    assert_eq!(records[0].id(), expected.id());
    assert!(std::ptr::eq(
        records[0].key(),
        external
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(expected.id())
            .unwrap()
            .as_ref()
    ));
    assert_eq!(graph.declared_identity_count(), 0);
    assert!(
        graph
            .records::<PersistentTypeId, SourceDeclarationKey>(
                IdentityLayer::Hir,
                &mut meter,
                &WirePath::root()
            )
            .unwrap()
            .is_empty()
    );
}

#[test]
fn closure_record_queries_share_work_and_allocation_limits() {
    let graph = validated_source_type_graph();
    let run = |meter: &mut BudgetMeter| {
        graph.closure_records::<PersistentTypeId, SourceDeclarationKey>(meter, &WirePath::root())
    };
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    run(&mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    run(&mut shared).unwrap();
    assert!(matches!(
        run(&mut shared),
        Err(IdentityValidationError::Resource(_))
    ));
    assert!(matches!(
        run(&mut BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        })),
        Err(IdentityValidationError::Resource(_))
    ));
}
