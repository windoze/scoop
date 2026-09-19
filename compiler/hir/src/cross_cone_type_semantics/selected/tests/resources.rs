use super::*;

#[test]
fn selected_resolution_checks_all_resource_families_before_resolving_ids() {
    let f = Fixture::new();
    let table = decoded(&[f.signature()]);
    for (limits, expected, at) in [
        (
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
            path(),
        ),
        (
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            path(),
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
            path(),
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
            path().index(0),
        ),
        (
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedNodes,
            path().index(0),
        ),
        (
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
            path().index(0),
        ),
        (
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
            path().index(0),
        ),
    ] {
        let mut resolver = f.resolver();
        resource(
            table
                .clone()
                .resolve(&mut resolver, &mut BudgetMeter::new(limits), &path())
                .unwrap_err(),
            expected,
            &at,
        );
        assert!(resolver.calls.is_empty());
    }
}

#[test]
fn wide_selected_table_is_rejected_before_reserving_or_resolving() {
    let f = Fixture::new();
    let wide = decoded(&vec![f.signature(); 128]);
    for (limits, expected) in [
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
        let mut resolver = f.resolver();
        let mut resources = BudgetMeter::new(limits);
        resource(
            wide.clone()
                .resolve(&mut resolver, &mut resources, &path())
                .unwrap_err(),
            expected,
            &path(),
        );
        assert!(resolver.calls.is_empty());
        assert_eq!(resources.usage().logical_heap_bytes, 0);
    }
}

#[test]
fn consecutive_selected_tables_cannot_reset_the_shared_meter() {
    let f = Fixture::new();
    let table = CanonicalSelectedExternalTypeUsesV1::try_new(vec![
        f.signature(),
        f.record(SelectedTypeUseV1::Representation { exact: f.owner }),
    ])
    .unwrap();
    let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 = parsed(&table);
    let mut baseline = meter();
    decoded
        .clone()
        .resolve(&mut f.resolver(), &mut baseline, &path())
        .unwrap();
    let usage = baseline.usage();
    for (limits, expected) in [
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units * 2 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: usage.logical_heap_bytes * 2 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
        (
            DecodeLimits {
                owned_bytes: usage.owned_bytes * 2 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
        ),
        (
            DecodeLimits {
                decoded_nodes: usage.decoded_nodes * 2 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedNodes,
        ),
        (
            DecodeLimits {
                decoded_edges: usage.decoded_edges * 2 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
        ),
    ] {
        let mut resources = BudgetMeter::new(limits);
        let mut resolver = f.resolver();
        assert_eq!(
            decoded
                .clone()
                .resolve(&mut resolver, &mut resources, &path())
                .unwrap(),
            table
        );
        assert_eq!(resources.usage(), usage);
        let error = decoded
            .clone()
            .resolve(&mut resolver, &mut resources, &path())
            .unwrap_err();
        let SelectedTypeUseResolutionError::Resource(error) = error else {
            panic!("{error:?}");
        };
        assert!(
            matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
            "{error:?}"
        );
        assert!(error.path().segments().starts_with(path().segments()));
        assert_eq!(error.byte_offset(), None);
    }
}

#[test]
fn selected_wire_decoder_enforces_cbor_depth_and_identity_leaf_size() {
    let f = Fixture::new();
    let bytes = encode(&f.signature()).unwrap();
    for (limits, expected) in [
        (
            DecodeLimits {
                cbor_nesting: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::CborNesting,
        ),
        (
            DecodeLimits {
                semantic_leaf_bytes: 31,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticLeafBytes,
        ),
    ] {
        let error =
            decode_canonical::<DecodedSelectedExternalTypeUseV1>(&bytes, limits).unwrap_err();
        assert!(
            matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
            "{error:?}"
        );
    }
    let decoded: DecodedSelectedExternalTypeUseV1 = decode_canonical(
        &bytes,
        DecodeLimits {
            semantic_leaf_bytes: 32,
            ..DecodeLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        decoded
            .resolve(&mut f.resolver(), &mut meter(), &path())
            .unwrap(),
        f.signature()
    );
}
