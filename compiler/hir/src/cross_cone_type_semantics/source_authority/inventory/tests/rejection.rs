use super::*;

#[test]
fn all_producers_reject_duplicate_keys() {
    let fixture = Resolver::new();
    let root = fixture.roots()[0];
    assert!(CanonicalSourceNominalIdsV1::try_new(vec![root, root], &mut meter()).is_err());
    let snapshot = fixture.snapshot();
    assert!(
        CanonicalTypeSourceNominalsV1::try_new(vec![snapshot.clone(), snapshot], &mut meter())
            .is_err()
    );
    let edge = fixture.edge();
    assert!(
        CanonicalNominalInheritanceEdgesV1::try_new(vec![edge.clone(), edge], &mut meter())
            .is_err()
    );
    let mut dependencies = fixture.dependencies();
    dependencies[1].exact = dependencies[0].exact;
    assert!(CanonicalTypeSectionDependencyFactsV1::try_new(dependencies, &mut meter()).is_err());
}

#[test]
fn all_readers_reject_duplicate_keys_without_deduplicating() {
    let mut fixture = Resolver::new();
    let root = fixture.roots()[0];
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&root, &root), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let snapshot = fixture.snapshot();
    let decoded: DecodedCanonicalTypeSourceNominalsV1 =
        decode_canonical(&two_records(&snapshot, &snapshot), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let edge = fixture.edge();
    let decoded: DecodedCanonicalNominalInheritanceEdgesV1 =
        decode_canonical(&two_records(&edge, &edge), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let mut dependencies = fixture.dependencies();
    dependencies[1].exact = dependencies[0].exact;
    let decoded: DecodedCanonicalTypeSectionDependencyFactsV1 = decode_canonical(
        &two_records(&dependencies[0], &dependencies[1]),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn readers_reject_noncanonical_order_instead_of_repairing_it() {
    let mut fixture = Resolver::new();
    let roots = fixture.roots();
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&roots[1], &roots[0]), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let mut dependencies = fixture.dependencies();
    dependencies.sort_unstable_by_key(|record| std::cmp::Reverse(record.exact));
    let decoded: DecodedCanonicalTypeSectionDependencyFactsV1 = decode_canonical(
        &two_records(&dependencies[0], &dependencies[1]),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn wire_products_reject_missing_extra_fields_and_invalid_nominal_tags() {
    for bytes in [vec![0xa1, 1, 0], vec![0xa3, 1, 0, 2, 0, 3, 0]] {
        assert!(
            decode_canonical::<DecodedTypeSectionDependencyFactV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
        assert!(
            decode_canonical::<DecodedTypeSourceNominalV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
    assert!(
        decode_canonical::<DecodedCanonicalSourceNominalIdsV1>(
            &[0x81, 0xa1, 0, 3],
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn all_inventories_fail_before_reference_lookup_when_budget_is_exhausted() {
    for limits in [
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
    ] {
        let mut fixture = Resolver::new();
        let roots = CanonicalSourceNominalIdsV1::try_new(fixture.roots(), &mut meter()).unwrap();
        let decoded = decoded::<DecodedCanonicalSourceNominalIdsV1>(&roots);
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
        let snapshots =
            CanonicalTypeSourceNominalsV1::try_new(vec![fixture.snapshot()], &mut meter()).unwrap();
        let decoded = super::decoded::<DecodedCanonicalTypeSourceNominalsV1>(&snapshots);
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
        let dependencies =
            CanonicalTypeSectionDependencyFactsV1::try_new(fixture.dependencies(), &mut meter())
                .unwrap();
        let decoded = super::decoded::<DecodedCanonicalTypeSectionDependencyFactsV1>(&dependencies);
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
        let edges = CanonicalNominalInheritanceEdgesV1::try_new(vec![fixture.edge()], &mut meter())
            .unwrap();
        let decoded = super::decoded::<DecodedCanonicalNominalInheritanceEdgesV1>(&edges);
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(SourceInventoryError::Resource(_))
        ));
        assert_eq!(fixture.queries, 0);
    }
}

#[test]
fn nested_origin_and_edge_budgets_are_charged_before_identity_queries() {
    let mut fixture = Resolver::new();
    let snapshots =
        CanonicalTypeSourceNominalsV1::try_new(vec![fixture.snapshot()], &mut meter()).unwrap();
    let decoded = decoded::<DecodedCanonicalTypeSourceNominalsV1>(&snapshots);
    let limits = DecodeLimits {
        owned_bytes: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
        Err(SourceInventoryError::Resource(_))
    ));
    let edges =
        CanonicalNominalInheritanceEdgesV1::try_new(vec![fixture.edge()], &mut meter()).unwrap();
    let decoded = super::decoded::<DecodedCanonicalNominalInheritanceEdgesV1>(&edges);
    let limits = DecodeLimits {
        decoded_edges: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut BudgetMeter::new(limits)),
        Err(SourceInventoryError::Resource(_))
    ));
    assert_eq!(fixture.queries, 0);
}

#[test]
fn unknown_source_and_dependency_identities_are_rejected() {
    let mut fixture = Resolver::new();
    fixture.reject_references = true;
    let roots = CanonicalSourceNominalIdsV1::try_new(fixture.roots(), &mut meter()).unwrap();
    assert!(matches!(
        decoded::<DecodedCanonicalSourceNominalIdsV1>(&roots).resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::Reference(_))
    ));
    let dependencies =
        CanonicalTypeSectionDependencyFactsV1::try_new(fixture.dependencies(), &mut meter())
            .unwrap();
    assert!(matches!(
        decoded::<DecodedCanonicalTypeSectionDependencyFactsV1>(&dependencies)
            .resolve(&mut fixture, &mut meter()),
        Err(SourceInventoryError::Reference(_))
    ));
}
