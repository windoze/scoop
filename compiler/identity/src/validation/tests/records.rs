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

    let records = graph
        .closure_records::<PersistentTypeId, SourceDeclarationKey>(&WirePath::root())
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
                &WirePath::root()
            )
            .unwrap()
            .is_empty()
    );
}
