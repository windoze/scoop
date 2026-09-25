use super::*;
use scoop_wire::{ResourceKind, WireErrorKind};

#[test]
fn source_calls_use_one_artifact_budget_with_exact_boundaries() {
    let fixture = Fixture::simple();
    let first = fixture.call(0, vec![unit_exact(); 2], unit_exact());
    let second = fixture.call(1, vec![unit_exact(); 2], unit_exact());
    let path = WirePath::root().field(10).index(0).field(5).index(1);
    let check = |site: &HirDependencyCallSiteV1, meter: &mut BudgetMeter| {
        site.validate_source_signature(fixture.target, fixture.metadata(), meter, &path)
    };
    let mut measured = meter();
    check(&first, &mut measured).unwrap();
    let cost = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost,
        ..DecodeLimits::default()
    });
    check(&first, &mut exact).unwrap();
    assert!(
        matches!(check(&second, &mut exact), Err(HirDependencyCallSignatureError::Resource(error))
        if error.path() == &path && matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
    let mut one_less = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost - 1,
        ..DecodeLimits::default()
    });
    assert!(
        matches!(check(&first, &mut one_less), Err(HirDependencyCallSignatureError::Resource(error))
        if error.path() == &path && matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
    let mut exact_pair = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost * 2,
        ..DecodeLimits::default()
    });
    check(&first, &mut exact_pair).unwrap();
    check(&second, &mut exact_pair).unwrap();
}

#[test]
fn source_call_rejects_zero_node_and_argument_table_budgets() {
    let fixture = Fixture::simple();
    let site = fixture.call(0, vec![unit_exact(); 2], unit_exact());
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            site.validate_source_signature(
                fixture.target,
                fixture.metadata(),
                &mut BudgetMeter::new(limits),
                &WirePath::root()
            ),
            Err(HirDependencyCallSignatureError::Resource(_))
        ));
    }
}
