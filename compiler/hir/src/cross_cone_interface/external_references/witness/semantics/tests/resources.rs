use super::*;
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

#[test]
fn dependency_witness_reuses_the_full_route_budget() {
    let fixture = chain();
    let path = WirePath::root().field(10).index(0).field(4).index(0);
    let validate = |meter: &mut BudgetMeter| {
        fixture
            .witness
            .validate_semantics(fixture.root, &fixture.authority, meter, &path)
    };
    let mut measured = route_meter();
    validate(&mut measured).unwrap();
    let work = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: work,
        ..DecodeLimits::default()
    });
    validate(&mut exact).unwrap();
    let DependencyBindingWitnessSemanticValidationError::Resource(error) =
        validate(&mut exact).unwrap_err()
    else {
        panic!("expected cumulative budget failure");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::ValidationWorkUnits,
            ..
        }
    ));
    assert_eq!(error.path(), &path);
}

#[test]
fn dependency_witness_depth_limit_preserves_reference_path() {
    let fixture = chain();
    let path = WirePath::root().field(10).index(2).field(4).index(1);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let DependencyBindingWitnessSemanticValidationError::Resource(error) = fixture
        .witness
        .validate_semantics(fixture.root, &fixture.authority, &mut meter, &path)
        .unwrap_err()
    else {
        panic!("expected depth budget failure");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticRecursion,
            ..
        }
    ));
    assert_eq!(error.path(), &path.field(2));
}
