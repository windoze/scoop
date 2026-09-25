use super::*;
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

fn replay(meter: &mut BudgetMeter) -> Result<(), Box<PublicExportBindingClosureValidationError>> {
    let fixture = route_fixture();
    let providers = [
        RouteProviderView {
            identity: fixture.direct,
            bindings: fixture.direct_interface.public_bindings(),
        },
        RouteProviderView {
            identity: fixture.terminal,
            bindings: fixture.terminal_interface.public_bindings(),
        },
    ];
    let direct = [fixture.direct];
    let path = WirePath::root();
    let authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &fixture.current_interface,
        &direct,
        &providers,
        meter,
        &path,
    )
    .map_err(PublicExportBindingClosureValidationError::Resource)?;
    assert_eq!(authority.binding_keys.len(), 3);
    fixture
        .current_interface
        .public_bindings()
        .validate_route_closure(fixture.current, &authority, meter, &path.field(9))
        .map_err(Box::new)
}

fn assert_limit(error: Box<PublicExportBindingClosureValidationError>, expected: ResourceKind) {
    let PublicExportBindingClosureValidationError::Resource(error) = *error else {
        panic!("expected resource exhaustion, got {error}");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)
    );
}

#[test]
fn route_index_and_replay_share_work_and_owned_storage_budgets() {
    let mut measured = route_meter();
    replay(&mut measured).unwrap();
    let usage = measured.usage();
    assert!(usage.owned_bytes > 0);
    for (exact, short, resource) in [
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: usage.validation_work_units - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
        (
            DecodeLimits {
                owned_bytes: usage.owned_bytes,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: usage.owned_bytes - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
        ),
    ] {
        replay(&mut BudgetMeter::new(exact)).unwrap();
        assert_limit(replay(&mut BudgetMeter::new(short)).unwrap_err(), resource);
    }
    let mut repeated = BudgetMeter::new(DecodeLimits {
        validation_work_units: 2 * usage.validation_work_units - 1,
        ..DecodeLimits::default()
    });
    replay(&mut repeated).unwrap();
    assert_limit(
        replay(&mut repeated).unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
    assert!(repeated.usage().validation_work_units >= usage.validation_work_units);
}

#[test]
fn index_limits_are_enforced_before_route_storage_is_allocated() {
    for (limits, resource) in [
        (
            DecodeLimits {
                semantic_table_entries: 2,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticTableEntries,
        ),
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
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
            ResourceKind::SemanticRecursion,
        ),
    ] {
        let mut meter = BudgetMeter::new(limits);
        assert_limit(replay(&mut meter).unwrap_err(), resource);
        assert_eq!(meter.usage().owned_bytes, 0);
    }
}

#[test]
fn provider_directories_account_for_their_actual_element_storage() {
    let path = WirePath::root();
    let mut values = Vec::<ConeIdentity>::new();
    let bytes = std::mem::size_of::<ConeIdentity>() as u64;
    let mut meter = BudgetMeter::new(DecodeLimits {
        owned_bytes: bytes - 1,
        ..DecodeLimits::default()
    });
    let error = reserve_route_slots(&mut values, 1, &mut meter, &path).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::OwnedBytes,
            ..
        }
    ));
    assert_eq!(values.capacity(), 0);
    let mut meter = BudgetMeter::new(DecodeLimits {
        owned_bytes: bytes,
        ..DecodeLimits::default()
    });
    reserve_route_slots(&mut values, 1, &mut meter, &path).unwrap();
    assert_eq!(meter.usage().owned_bytes, bytes);
}
