use super::*;

#[test]
fn canonical_selected_targets_preserve_provider_use_and_exact_distinctions() {
    let f = Fixture::new();
    let mut records: Vec<_> = f
        .cases()
        .into_iter()
        .flat_map(|(usage, _)| {
            [
                f.record(usage),
                SelectedExternalTypeUseV1::new(f.alternate, usage),
            ]
        })
        .collect();
    records.push(f.record(SelectedTypeUseV1::Signature { exact: f.derived }));
    records.reverse();
    let canonical = CanonicalSelectedExternalTypeUsesV1::try_new(records.clone()).unwrap();
    assert_eq!(canonical.records().len(), 27);
    for record in &records {
        assert!(canonical.records().contains(record));
    }
    let keys: Vec<_> = canonical
        .records()
        .iter()
        .map(|record| encode(record).unwrap())
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 = parsed(&canonical);
    assert_eq!(encode(&decoded).unwrap(), encode(&canonical).unwrap());
    assert_eq!(
        decoded
            .resolve(&mut f.resolver(), &mut meter(), &path())
            .unwrap(),
        canonical
    );
}

#[test]
fn complete_canonical_encoding_orders_simple_use_before_compound_use() {
    let f = Fixture::new();
    let construction = f.record(SelectedTypeUseV1::Construct {
        exact: f.owner,
        declaration: SelectedTypeConstructionV1::Constructor(f.constructor),
    });
    let shape = f.record(SelectedTypeUseV1::ShapeSupport { exact: f.owner });
    let table = CanonicalSelectedExternalTypeUsesV1::try_new(vec![construction, shape]).unwrap();
    // The a2/a3 product prefix precedes the use's numeric tag in canonical bytes.
    assert_eq!(table.records(), &[shape, construction]);
}

#[test]
fn producer_rejects_duplicates_and_reader_never_repairs_target_order() {
    let f = Fixture::new();
    let first = f.signature();
    assert!(matches!(
        CanonicalSelectedExternalTypeUsesV1::try_new(vec![first, first]),
        Err(SelectedTypeUseBuildError::Duplicate { index: 1 }),
    ));
    let repeated = decoded(&[first, first]);
    assert!(matches!(
        repeated.resolve(&mut f.resolver(), &mut meter(), &path()),
        Err(SelectedTypeUseResolutionError::Duplicate { index: 1 }),
    ));
    let other = f.record(SelectedTypeUseV1::Representation { exact: f.owner });
    let ordered = CanonicalSelectedExternalTypeUsesV1::try_new(vec![other, first]).unwrap();
    let reversed: Vec<_> = ordered.records().iter().rev().copied().collect();
    let parsed = decoded(&reversed);
    assert_eq!(
        encode(&parsed).unwrap(),
        encode(&Sequence(&reversed)).unwrap()
    );
    assert!(matches!(
        parsed.resolve(&mut f.resolver(), &mut meter(), &path()),
        Err(SelectedTypeUseResolutionError::NonCanonicalOrder { index: 1 }),
    ));
}
