use super::*;

#[test]
fn representation_join_meters_actual_leaves_nodes_work_and_wide_fields() {
    let (fixture, _) = fixtures::structure(unit(), 2);
    let table = fixture.table();
    let mut baseline = meter();
    table
        .validate_source_semantics(&fixture, &mut baseline, &path())
        .unwrap();
    for (limits, expected) in [
        (
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedNodes,
        ),
        (
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
        (
            DecodeLimits {
                semantic_recursion: 3,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
        ),
        (
            DecodeLimits {
                semantic_leaf_bytes: 10,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticLeafBytes,
        ),
        (
            DecodeLimits {
                validation_work_units: baseline.usage().validation_work_units - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
    ] {
        resource(
            table
                .validate_source_semantics(&fixture, &mut BudgetMeter::new(limits), &path())
                .unwrap_err(),
            expected,
        );
    }
    // "types.scoop" is the longest actual text leaf; enclosing products are larger.
    table
        .validate_source_semantics(
            &fixture,
            &mut BudgetMeter::new(DecodeLimits {
                semantic_leaf_bytes: 11,
                ..DecodeLimits::default()
            }),
            &path(),
        )
        .unwrap();
    let (wide, _) = fixtures::structure(unit(), 128);
    resource(
        wide.table()
            .validate_source_semantics(
                &wide,
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_table_entries: 64,
                    ..DecodeLimits::default()
                }),
                &path(),
            )
            .unwrap_err(),
        ResourceKind::SemanticTableEntries,
    );
}

#[test]
fn representation_signatures_share_depth_and_sibling_work_limits() {
    let mut deep = unit();
    for _ in 0..32 {
        deep = SignatureTypeKey::RawPointer(Box::new(deep));
    }
    let (fixture, _) = fixtures::structure(deep, 1);
    resource(
        fixture
            .table()
            .validate_source_semantics(
                &fixture,
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_recursion: 16,
                    ..DecodeLimits::default()
                }),
                &path(),
            )
            .unwrap_err(),
        ResourceKind::SemanticRecursion,
    );
    let wide = SignatureTypeKey::Tuple(NonEmptyVec::new(vec![unit(); 4096]).unwrap());
    let (fixture, _) = fixtures::structure(wide, 1);
    let table = fixture.table();
    resource(
        table
            .validate_source_semantics(
                &fixture,
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_table_entries: 128,
                    ..DecodeLimits::default()
                }),
                &path(),
            )
            .unwrap_err(),
        ResourceKind::SemanticTableEntries,
    );
    resource(
        table
            .validate_source_semantics(
                &fixture,
                &mut BudgetMeter::new(DecodeLimits {
                    validation_work_units: 2048,
                    ..DecodeLimits::default()
                }),
                &path(),
            )
            .unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
}
