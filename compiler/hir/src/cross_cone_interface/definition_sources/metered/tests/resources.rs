use super::*;

#[test]
fn metered_source_reader_rejects_each_resolution_resource_before_identity_lookup() {
    let path = WirePath::root().field(8);
    for (limits, expected, table) in [
        (
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
            true,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
            true,
        ),
        (
            DecodeLimits {
                semantic_recursion: 2,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
            false,
        ),
        (
            DecodeLimits {
                semantic_leaf_bytes: SOURCE_PATH.len() as u64 - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticLeafBytes,
            false,
        ),
        (
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedNodes,
            false,
        ),
        (
            DecodeLimits {
                decoded_edges: 1,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
            false,
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
            false,
        ),
        (
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            false,
        ),
    ] {
        let mut foundation = Foundation::default();
        let error = one()
            .resolve_metered(&mut foundation, &mut BudgetMeter::new(limits), &path)
            .unwrap_err();
        resource(
            error,
            expected,
            &if table {
                path.clone()
            } else {
                path.clone().index(0)
            },
        );
        assert_eq!((foundation.cone_calls, foundation.context_calls), (0, 0));
    }
}
#[test]
fn metered_source_reader_charges_repeated_resolution_to_the_same_budget() {
    let path = WirePath::root().field(8);
    let mut baseline = meter();
    one()
        .resolve_metered(&mut Foundation::default(), &mut baseline, &path)
        .unwrap();
    let work = baseline.usage().validation_work_units;
    assert!(work > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: work * 2 - 1,
        ..DecodeLimits::default()
    });
    let mut foundation = Foundation::default();
    one()
        .resolve_metered(&mut foundation, &mut shared, &path)
        .unwrap();
    assert_eq!(shared.usage().validation_work_units, work);
    let error = one()
        .resolve_metered(&mut foundation, &mut shared, &path)
        .unwrap_err();
    resource(error, ResourceKind::ValidationWorkUnits, &path.index(0));
    assert_eq!((foundation.cone_calls, foundation.context_calls), (1, 1));
}
#[test]
fn source_path_leaf_limit_does_not_treat_the_whole_origin_product_as_text() {
    let source = source(1);
    let leaf = SOURCE_PATH.len() as u64;
    assert!(encoded_length(&source).unwrap() > leaf);
    let mut resources = BudgetMeter::new(DecodeLimits {
        semantic_leaf_bytes: leaf,
        ..DecodeLimits::default()
    });
    let table = one()
        .resolve_metered(
            &mut Foundation::default(),
            &mut resources,
            &WirePath::root(),
        )
        .unwrap();
    assert_eq!(table.sources(), &[source]);
    let mut foundation = Foundation::default();
    let error = one()
        .resolve_metered(
            &mut foundation,
            &mut BudgetMeter::new(DecodeLimits {
                semantic_leaf_bytes: leaf - 1,
                ..DecodeLimits::default()
            }),
            &WirePath::root(),
        )
        .unwrap_err();
    resource(
        error,
        ResourceKind::SemanticLeafBytes,
        &WirePath::root().index(0),
    );
    assert_eq!(foundation.cone_calls, 0);
}
