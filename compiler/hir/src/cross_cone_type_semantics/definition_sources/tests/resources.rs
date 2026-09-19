use super::*;

#[test]
fn declared_source_seen_work_is_charged_before_allocating_or_calling_authority() {
    let fixture = Fixture::default();
    let declared = CanonicalExportDefinitionSourcesV1::try_new(
        (0..128)
            .map(|start| origin(ConeIdentity::CORE, start))
            .collect(),
    )
    .unwrap();
    let path = WirePath::root().field(19);
    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: declared.sources().len() as u64 - 1,
        ..DecodeLimits::default()
    });
    let mut authority = authority::Authority::new(&fixture.expected, &meter, &path);
    let error = fixture
        .inputs()
        .validate_definition_sources(&declared, &mut authority, &mut meter, &path)
        .unwrap_err();
    let TypeDefinitionSourceClosureError::Resource(resource) = &error else {
        panic!("expected declared-source preflight failure: {error:?}");
    };
    assert_eq!(resource.path(), &path.clone().field(7));
    assert_resource(error, ResourceKind::ValidationWorkUnits);
    assert_eq!(authority.calls, 0);
    assert_eq!(meter.usage().logical_heap_bytes, 0);
    assert_eq!(meter.usage().decoded_nodes, 0);
}

#[test]
fn definition_source_closure_respects_shared_work_and_typed_table_budgets() {
    let (fixture, _) = representations::fixture(2);
    let declared = fixture.declared();
    let (_, work) = fixture
        .validate(&declared, DecodeLimits::default())
        .unwrap();
    assert_resource(
        fixture
            .validate(
                &declared,
                DecodeLimits {
                    validation_work_units: work - 1,
                    ..DecodeLimits::default()
                },
            )
            .unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
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
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
        (
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticLeafBytes,
        ),
        (
            DecodeLimits {
                semantic_table_entries: 1,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
        ),
    ] {
        assert_resource(fixture.validate(&declared, limits).unwrap_err(), expected);
    }
    let (wide, _) = representations::fixture(128);
    assert_resource(
        wide.validate(
            &wide.declared(),
            DecodeLimits {
                semantic_table_entries: 64,
                ..DecodeLimits::default()
            },
        )
        .unwrap_err(),
        ResourceKind::SemanticTableEntries,
    );
}
#[test]
fn nested_source_depth_and_default_body_depth_use_the_shared_recursion_limit() {
    for fixture in [nested::fixture(), defaults::fixture()] {
        assert_resource(
            fixture
                .validate(
                    &fixture.declared(),
                    DecodeLimits {
                        semantic_recursion: 1,
                        ..DecodeLimits::default()
                    },
                )
                .unwrap_err(),
            ResourceKind::SemanticRecursion,
        );
    }
}
