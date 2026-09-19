use super::*;

#[test]
fn recursive_semantic_edges_are_exact_and_every_incoming_edge_replays() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let a = provider.add("First", true);
    let b = provider.add("Second", true);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let first = request(a);
    let second = request(b);
    let wire = consumer.section(vec![first, second]);
    let mut uses = Uses::new(&[first]);
    uses.edges = vec![
        (first, Uses::new(&[second, second]).roots),
        (second, Uses::new(&[first]).roots),
    ];
    let checked = check(&consumer, &wire, &public, &[&terminal], &uses).unwrap();
    assert_eq!(checked.selected().len(), 2);
    assert_eq!(uses.edge_calls.get(), 3);
    let missing = consumer.section(vec![first]);
    assert!(
        matches!(check(&consumer, &missing, &public, &[&terminal], &uses), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::Inventory))
    );
    uses.edges[0].1[1].permitted = false;
    assert!(
        matches!(check(&consumer, &wire, &public, &[&terminal], &uses), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::Source("semantic edge denied")))
    );
}

#[test]
fn recursive_selection_depth_is_metered_after_cycle_deduplication() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let nodes: Vec<_> = (0..20)
        .map(|i| provider.add(&format!("Depth{i}"), true))
        .collect();
    let requests: Vec<_> = nodes.into_iter().map(request).collect();
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let wire = consumer.section(requests.clone());
    let mut uses = Uses::new(&requests[..1]);
    uses.edges = requests
        .windows(2)
        .map(|pair| (pair[0], Uses::new(&pair[1..]).roots))
        .collect();
    let mut budget = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 14,
        ..DecodeLimits::default()
    });
    let error = wire
        .validate_semantics(
            public_proof(&public, consumer.provider),
            &[&terminal],
            &consumer,
            &mut consumer.source.clone(),
            &mut DefaultAuthority::new(&consumer),
            &uses,
            &mut budget,
            &path(),
        )
        .unwrap_err();
    assert!(
        matches!(error, TypeSectionSemanticValidationError::Selected(error)
        if matches!(*error, TypeSelectionValidationError::Resource(ref error)
            if matches!(error.kind(), scoop_wire::WireErrorKind::LimitExceeded { resource: scoop_wire::ResourceKind::SemanticRecursion, .. })))
    );
}

#[test]
fn local_uses_do_not_become_persistent_external_records_and_provenance_is_checked() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    let root = fixture.add("Local", true);
    let local = request(root);
    let public = public();
    let valid = fixture.section(vec![]);
    let uses = Uses::new(&[local]);
    assert!(
        check(&fixture, &valid, &public, &[], &uses)
            .unwrap()
            .selected()
            .is_empty()
    );
    let invalid = fixture.section(vec![local]);
    assert!(check(&fixture, &invalid, &public, &[], &uses).is_err());
    let key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![]),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("missingDefault").unwrap(),
        0,
        None,
        vec![],
    );
    let owner = CallableTemplateOrigin::Function(
        PersistentFunctionId::from_source_declaration(&key).unwrap(),
    );
    let mut uses = Uses::new(&[local]);
    uses.roots[0].origin = TypeSectionCommittedRootOriginV1::ProtectedDefault {
        provider: ConeIdentity::CORE,
        key: ProtectedDefaultTemplateKeyV1::try_new(owner, 0).unwrap(),
    };
    assert!(
        matches!(check(&fixture, &valid, &public, &[], &uses), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::DefaultOrigin))
    );
    assert_eq!(uses.root_calls.get(), 0);
    let default_origin = uses.roots[0].origin;
    uses.roots[0].origin = TypeSectionCommittedRootOriginV1::Source;
    uses.edges = vec![(
        local,
        vec![Root {
            request: local,
            origin: default_origin,
            permitted: true,
        }],
    )];
    assert!(
        matches!(check(&fixture, &valid, &public, &[], &uses), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::DefaultOrigin))
    );
    assert_eq!(uses.edge_calls.get(), 0);
}

fn request(node: Node) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        ConeIdentity::CORE,
        SelectedTypeUseV1::Representation { exact: node.exact },
    )
}
