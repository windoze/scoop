use super::*;

#[test]
fn metered_table_keeps_canonical_order_and_rejects_duplicates_without_repair() {
    let mut fixture = Fixture::new(SourceNominalKind::Interface);
    let extra = Arc::new(source_key("Other", SourceNominalKind::Interface));
    let records = [&fixture.key, extra.as_ref()]
        .into_iter()
        .map(|key| {
            NominalRepresentationSupportV1::try_new(
                key,
                fixture.access.clone(),
                NominalRepresentationShapeV1::Interface,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let canonical = CanonicalNominalRepresentationSupportV1::try_new(records).unwrap();
    let decoded: DecodedCanonicalNominalRepresentationSupportV1 = parsed(&canonical);
    let mut resolver = Counting::new(&mut fixture);
    resolver.extra_source = Some(extra);
    assert_eq!(
        decoded
            .clone()
            .resolve_metered(&mut resolver, &mut meter(), &path())
            .unwrap(),
        canonical
    );
    assert_eq!(
        decoded.resolve(&mut resolver, &mut meter()).unwrap(),
        canonical
    );
    let ordered = canonical.records();
    for records in [
        ordered.iter().rev().cloned().collect(),
        vec![ordered[0].clone(); 2],
    ] {
        let decoded: DecodedCanonicalNominalRepresentationSupportV1 = parsed(&Sequence(records));
        assert!(matches!(
            decoded.resolve_metered(&mut resolver, &mut meter(), &path()),
            Err(NominalRepresentationTableResolutionError::Order(
                NominalRepresentationTableOrderError { index: 1, .. }
            ))
        ));
    }
    assert!(CanonicalNominalRepresentationSupportV1::try_new(vec![ordered[0].clone(); 2]).is_err());
    resolver.calls.clear();
    let empty: DecodedCanonicalNominalRepresentationSupportV1 = parsed(&Sequence(vec![]));
    assert!(
        empty
            .resolve_metered(&mut resolver, &mut meter(), &path())
            .unwrap()
            .records()
            .is_empty()
    );
    assert!(resolver.calls.is_empty());
}

#[test]
fn wide_table_and_late_wide_record_fail_before_any_resolver_call() {
    let (mut fixture, record) = fixtures::structure(unit(), 1);
    let wide: DecodedCanonicalNominalRepresentationSupportV1 =
        parsed(&Sequence(vec![record.clone(); 128]));
    for (limits, kind) in [
        (
            DecodeLimits {
                semantic_table_entries: 127,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
        ),
        (
            DecodeLimits {
                validation_work_units: 127,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
    ] {
        let mut resolver = Counting::new(&mut fixture);
        let mut resources = BudgetMeter::new(limits);
        table_resource(
            wide.clone()
                .resolve_metered(&mut resolver, &mut resources, &path())
                .unwrap_err(),
            kind,
            &path(),
        );
        assert!(resolver.calls.is_empty());
        assert_eq!(resources.usage().logical_heap_bytes, 0);
    }
    let (_, late) = fixtures::structure(unit(), 128);
    let decoded: DecodedCanonicalNominalRepresentationSupportV1 =
        parsed(&Sequence(vec![record, late]));
    let mut resolver = Counting::new(&mut fixture);
    let mut resources = BudgetMeter::new(DecodeLimits {
        semantic_table_entries: 64,
        ..DecodeLimits::default()
    });
    table_resource(
        decoded
            .resolve_metered(&mut resolver, &mut resources, &path())
            .unwrap_err(),
        ResourceKind::SemanticTableEntries,
        &path().index(1).field(3).field(1),
    );
    assert!(resolver.calls.is_empty());
}

fn table_resource(
    error: NominalRepresentationTableResolutionError<&'static str>,
    expected: ResourceKind,
    at: &WirePath,
) {
    let NominalRepresentationTableResolutionError::Resource(error) = error else {
        panic!("{error:?}");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
        "{error:?}"
    );
    assert_eq!(error.path(), at);
    assert_eq!(error.byte_offset(), None);
}
