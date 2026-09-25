use super::*;
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

fn validate(meter: &mut BudgetMeter) -> Result<(), PublicExportBindingClosureValidationError> {
    let fixture = chain();
    fixture.current_surface.validate_route_closure(
        fixture.current,
        &fixture.authority,
        meter,
        &WirePath::root().field(9),
    )
}

fn resource(error: PublicExportBindingClosureValidationError, expected: ResourceKind) {
    let PublicExportBindingClosureValidationError::Resource(error) = error else {
        panic!("expected resource error, got {error}");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)
    );
}

#[test]
fn route_work_is_inclusive_and_accumulates_across_repeated_validation() {
    let mut measured = route_meter();
    validate(&mut measured).unwrap();
    let work = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: work,
        ..DecodeLimits::default()
    });
    validate(&mut exact).unwrap();
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: work - 1,
        ..DecodeLimits::default()
    });
    resource(
        validate(&mut short).unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
    let mut repeated = BudgetMeter::new(DecodeLimits {
        validation_work_units: 2 * work - 1,
        ..DecodeLimits::default()
    });
    validate(&mut repeated).unwrap();
    resource(
        validate(&mut repeated).unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
    assert!(repeated.usage().validation_work_units >= work);
}

#[test]
fn route_graph_limits_cover_nodes_edges_tables_and_depth() {
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
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
        ),
        (
            DecodeLimits {
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
    ] {
        resource(
            validate(&mut BudgetMeter::new(limits)).unwrap_err(),
            expected,
        );
    }
}

#[test]
fn route_depth_failure_identifies_the_hop_sequence() {
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let PublicExportBindingClosureValidationError::Resource(error) =
        validate(&mut meter).unwrap_err()
    else {
        panic!("expected depth limit");
    };
    assert_eq!(
        error.path(),
        &WirePath::root()
            .field(9)
            .index(0)
            .field(2)
            .field(1)
            .index(0)
            .field(2)
    );
}

#[test]
fn suffix_comparison_visits_the_last_hop_before_rejecting_a_common_prefix() {
    let fixture = chain();
    let route = ReexportRouteV1::try_new(
        fixture.direct,
        vec![
            ReexportRouteHopV1::new(fixture.direct, fixture.direct_binding),
            ReexportRouteHopV1::new(fixture.terminal, fixture.terminal_binding),
        ],
    )
    .unwrap();
    let mut different = route.hops().to_vec();
    different[1] = ReexportRouteHopV1::new(fixture.terminal, fixture.current_binding);
    let routes = CanonicalReexportRoutesV1::try_new(vec![route.clone()]).unwrap();
    let path = WirePath::root().field(9);
    let mut measured = route_meter();
    assert!(
        !routes
            .contains_exact_suffix_metered(&different, &mut measured, &path)
            .unwrap()
    );
    let work = measured.usage().validation_work_units;
    assert_eq!(work, 4);
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: work - 1,
        ..DecodeLimits::default()
    });
    let error = routes
        .contains_exact_suffix_metered(&different, &mut short, &path)
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::ValidationWorkUnits,
            ..
        }
    ));
    assert!(
        routes
            .contains_exact_suffix_metered(route.hops(), &mut route_meter(), &path)
            .unwrap()
    );
}
