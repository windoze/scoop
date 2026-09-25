use super::*;

#[test]
fn all_producers_reject_duplicate_keys() {
    let fixture = Resolver::new();
    let root = fixture.roots()[0];
    assert!(CanonicalSourceNominalIdsV1::try_new(vec![root, root]).is_err());
    let snapshot = fixture.snapshot();
    assert!(CanonicalTypeSourceNominalsV1::try_new(vec![snapshot.clone(), snapshot]).is_err());
    let edge = fixture.edge();
    assert!(CanonicalNominalInheritanceEdgesV1::try_new(vec![edge.clone(), edge]).is_err());
    let mut dependencies = fixture.dependencies();
    dependencies[1].exact = dependencies[0].exact;
    assert!(CanonicalTypeSectionDependencyFactsV1::try_new(dependencies).is_err());
}

#[test]
fn all_readers_reject_duplicate_keys_without_deduplicating() {
    let mut fixture = Resolver::new();
    let root = fixture.roots()[0];
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&root, &root)).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let snapshot = fixture.snapshot();
    let decoded: DecodedCanonicalTypeSourceNominalsV1 =
        decode_canonical(&two_records(&snapshot, &snapshot)).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let edge = fixture.edge();
    let decoded: DecodedCanonicalNominalInheritanceEdgesV1 =
        decode_canonical(&two_records(&edge, &edge)).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let mut dependencies = fixture.dependencies();
    dependencies[1].exact = dependencies[0].exact;
    let decoded: DecodedCanonicalTypeSectionDependencyFactsV1 =
        decode_canonical(&two_records(&dependencies[0], &dependencies[1])).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn readers_reject_noncanonical_order_instead_of_repairing_it() {
    let mut fixture = Resolver::new();
    let roots = fixture.roots();
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&roots[1], &roots[0])).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
    let mut dependencies = fixture.dependencies();
    dependencies.sort_unstable_by_key(|record| std::cmp::Reverse(record.exact));
    let decoded: DecodedCanonicalTypeSectionDependencyFactsV1 =
        decode_canonical(&two_records(&dependencies[0], &dependencies[1])).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn wire_products_reject_missing_extra_fields_and_invalid_nominal_tags() {
    for bytes in [vec![0xa1, 1, 0], vec![0xa3, 1, 0, 2, 0, 3, 0]] {
        assert!(decode_canonical::<DecodedTypeSectionDependencyFactV1>(&bytes).is_err());
        assert!(decode_canonical::<DecodedTypeSourceNominalV1>(&bytes).is_err());
    }
    assert!(decode_canonical::<DecodedCanonicalSourceNominalIdsV1>(&[0x81, 0xa1, 0, 3]).is_err());
}

#[test]
fn unknown_source_and_dependency_identities_are_rejected() {
    let mut fixture = Resolver::new();
    fixture.reject_references = true;
    let roots = CanonicalSourceNominalIdsV1::try_new(fixture.roots()).unwrap();
    assert!(matches!(
        decoded::<DecodedCanonicalSourceNominalIdsV1>(&roots).resolve(&mut fixture),
        Err(SourceInventoryError::Reference(_))
    ));
    let dependencies =
        CanonicalTypeSectionDependencyFactsV1::try_new(fixture.dependencies()).unwrap();
    assert!(matches!(
        decoded::<DecodedCanonicalTypeSectionDependencyFactsV1>(&dependencies)
            .resolve(&mut fixture),
        Err(SourceInventoryError::Reference(_))
    ));
}
