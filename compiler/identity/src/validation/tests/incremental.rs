use super::*;

#[test]
fn extending_a_shared_graph_preserves_the_original_records() {
    let graph = validated_source_type_graph();
    let snapshot = graph.clone();
    assert!(Rc::ptr_eq(&graph.candidates, &snapshot.candidates));

    let exact = decoded_exact_type();
    let mut pending = PendingIdentityValidation::from_graph(graph);
    pending.register(IdentityLayer::Mir, &exact).unwrap();
    pending.resolve(&exact).unwrap();
    let extended = pending.finish().unwrap();

    assert_eq!(snapshot.declared_identity_count(), 1);
    assert_eq!(extended.declared_identity_count(), 2);
    assert_eq!(
        extended
            .records::<PersistentExactTypeId, ExactTypeKey>(IdentityLayer::Mir, &WirePath::root())
            .unwrap(),
        vec![exact_type_record()]
    );
    assert!(
        snapshot
            .canonical_key::<PersistentExactTypeId, ExactTypeKey>(exact_type_record().id())
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &snapshot
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(source_type_record().id())
            .unwrap(),
        &extended
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(source_type_record().id())
            .unwrap()
    ));
}

#[test]
fn resolved_candidates_still_participate_in_new_cycles() {
    let first = source_type_record();
    let second = exact_type_record();
    let first_node = IdentityNode::trusted(first.id());
    let second_node = IdentityNode::trusted(second.id());
    for reverse in [false, true] {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_canonical(IdentityLayer::Hir, first.clone())
            .unwrap();
        pending
            .register_canonical(IdentityLayer::Hir, second.clone())
            .unwrap();
        let mut edge = |source, target_is_first| {
            let mut resolver = PendingIdentityResolver {
                current: source,
                candidates: &mut pending.candidates,
                resource_error: &mut pending.resource_error,
                resource_path: &pending.resource_path,
            };
            if target_is_first {
                resolver.resolve(first.id().into()).unwrap();
            } else {
                resolver.resolve(second.id().into()).unwrap();
            }
        };
        if reverse {
            edge(second_node, true);
            edge(first_node, false);
        } else {
            edge(first_node, false);
            edge(second_node, true);
        }
        let expected = first_node.min(second_node);
        assert!(matches!(
            pending.finish(),
            Err(IdentityValidationError::DependencyCycle { kind, id })
                if kind == expected.kind && id == expected.bytes
        ));
    }
}

#[test]
fn extending_a_validated_graph_rejects_a_new_self_cycle() {
    let mut pending = PendingIdentityValidation::from_graph(validated_source_type_graph());
    let record = exact_type_record();
    pending
        .register_canonical(IdentityLayer::Mir, record.clone())
        .unwrap();
    let mut resolver = PendingIdentityResolver {
        current: IdentityNode::trusted(record.id()),
        candidates: &mut pending.candidates,
        resource_error: &mut pending.resource_error,
        resource_path: &pending.resource_path,
    };
    resolver.resolve(record.id().into()).unwrap();
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::DependencyCycle { .. })
    ));
}

#[test]
fn repeated_session_import_can_be_extended_without_changing_world_ids() {
    let graph = validated_source_type_graph();
    let fingerprint = SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]);
    let mut session = SemanticIdentitySession::new();
    let (first, _, _) = session
        .import(ConeIdentity::CORE, fingerprint, &graph)
        .unwrap()
        .into_parts();
    let (repeated, _, _) = session
        .import(ConeIdentity::CORE, fingerprint, &graph.clone())
        .unwrap()
        .into_parts();
    assert_eq!(
        first.get(source_type_record().id()),
        repeated.get(source_type_record().id())
    );

    let mut snapshot = session.try_clone().unwrap();
    let mut pending = PendingIdentityValidation::from_graph(graph.clone());
    pending
        .register_canonical(IdentityLayer::Mir, exact_type_record())
        .unwrap();
    let extended = pending.finish().unwrap();
    let (hir, mir, _) = session
        .import(ConeIdentity::CORE, fingerprint, &extended)
        .unwrap()
        .into_parts();
    assert_eq!(
        first.get(source_type_record().id()),
        hir.get(source_type_record().id())
    );
    assert!(mir.get(exact_type_record().id()).is_some());
    assert_eq!(session.entity_count(), 2);
    assert_eq!(snapshot.entity_count(), 1);
    let (_, previous_mir, _) = snapshot
        .import(ConeIdentity::CORE, fingerprint, &graph)
        .unwrap()
        .into_parts();
    assert!(previous_mir.get(exact_type_record().id()).is_none());
}

#[test]
fn shared_graph_merges_and_session_imports_still_reject_conflicting_keys() {
    let original = validated_source_type_graph();
    let node = IdentityNode::trusted(source_type_record().id());
    // Model a collision at the graph merge boundary without relying on a hash collision.
    for key in [
        Arc::new(source_function_record().key().clone()) as Arc<dyn ErasedCanonicalKey>,
        Arc::new(exact_type_record().key().clone()) as Arc<dyn ErasedCanonicalKey>,
    ] {
        let mut conflicting = original.clone();
        Rc::make_mut(&mut conflicting.candidates)
            .get_mut(&node)
            .unwrap()
            .canonical_key = Some(key);
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(&original)
            .unwrap();
        assert!(matches!(
            pending.register_external_graph_authorities(&conflicting),
            Err(IdentityValidationError::IdentityCollision { .. })
        ));
        let fingerprint = SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]);
        let mut session = SemanticIdentitySession::new();
        session
            .import(ConeIdentity::CORE, fingerprint, &original)
            .unwrap();
        assert!(matches!(
            session.import(ConeIdentity::CORE, fingerprint, &conflicting),
            Err(SemanticIdentityImportError::IdentityConflict { .. })
        ));
        assert_eq!(session.entity_count(), 1);
        session
            .import(ConeIdentity::CORE, fingerprint, &original)
            .unwrap();
    }
}
