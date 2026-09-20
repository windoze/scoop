use super::*;

#[test]
fn source_body_uses_the_callers_remaining_budget() {
    with_hir_source(SOURCE, |output, _| {
        let owner = function(output.output().export.module(), "Base.callback");
        let mut measured = meter();
        Body::from_ordinary_hir(output, owner, 1, &mut measured).unwrap();
        let work = measured.usage().validation_work_units;
        assert!(work > 0);
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        Body::from_ordinary_hir(output, owner, 1, &mut shared).unwrap();
        let error = Body::from_ordinary_hir(output, owner, 1, &mut shared).unwrap_err();
        assert_resource(&error, ResourceKind::ValidationWorkUnits);
        assert!(shared.usage().validation_work_units >= work);
    });
}

#[test]
fn source_body_checks_allocation_and_recursion_limits_before_projection() {
    with_hir_source(SOURCE, |output, _| {
        let owner = function(output.output().export.module(), "Base.callback");
        for (limits, kind) in [
            (
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::DecodedNodes,
            ),
            (
                DecodeLimits {
                    owned_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::OwnedBytes,
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
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticRecursion,
            ),
            (
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticTableEntries,
            ),
            (
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::ValidationWorkUnits,
            ),
            (
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticLeafBytes,
            ),
        ] {
            let error = Body::from_ordinary_hir(output, owner, 1, &mut BudgetMeter::new(limits))
                .unwrap_err();
            assert_resource(&error, kind);
        }
    });
}

fn assert_resource(error: &Error, expected: ResourceKind) {
    let resource = error
        .resource_error()
        .unwrap_or_else(|| panic!("expected resource error: {error:?}"));
    assert!(
        matches!(resource.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
        "{error:?}"
    );
}

#[test]
fn source_body_charges_literal_bytes_before_copying_private_defaults() {
    with_hir_source(SOURCE, |output, _| {
        let owner = function(output.output().export.module(), "Base.hidden");
        let limits = DecodeLimits {
            semantic_leaf_bytes: "src/main.scoop".len() as u64,
            ..DecodeLimits::default()
        };
        let error =
            Body::from_ordinary_hir(output, owner, 0, &mut BudgetMeter::new(limits)).unwrap_err();
        assert_resource(&error, ResourceKind::SemanticLeafBytes);
    });
}
