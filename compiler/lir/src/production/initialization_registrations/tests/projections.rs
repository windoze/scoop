use super::*;
use crate::{DecodedStrongInitializationUnitSemanticProjectionV1, StrongSemanticProjectionError};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

fn decode(
    plan: &StrongInitializationUnitSemanticPlanV1,
) -> DecodedStrongInitializationUnitSemanticProjectionV1 {
    decode_canonical(
        &encode(&plan.semantic_projection()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn unit_projection_preserves_eager_schedule_and_complete_dependency_order() {
    let mut fixture = Fixture::eager();
    let dependency = fixture.add_eager_dependency("dependency");
    fixture.units[fixture.unit].dependencies.push(dependency);
    let plans = fixture.build().unwrap();
    let expected = plans
        .units()
        .iter()
        .find(|plan| plan.unit() == fixture.unit_id)
        .unwrap();
    let bytes = encode(&expected.semantic_projection()).unwrap();
    assert_eq!(bytes[0], 0xa8);
    assert_eq!(encode(&decode(expected)).unwrap(), bytes);
    decode(expected)
        .validate_against(expected, &mut meter())
        .unwrap();
    let changed = StrongInitializationUnitSemanticPlanV1::from_artifact(
        expected.unit(),
        expected.diagnostic_path().to_owned(),
        StrongInitializationSchedulePlanV1::LazyAccess,
        expected.storage(),
        expected.failure_root(),
        expected.initializer(),
        expected.ensure(),
        expected.dependencies().to_vec(),
    );
    assert!(matches!(
        decode(&changed).validate_against(expected, &mut meter()),
        Err(StrongSemanticProjectionError::Mismatch)
    ));
}

#[test]
fn unit_projection_rejects_wrong_failure_root_and_enforces_shared_budget() {
    let fixture = Fixture::lazy();
    let plans = fixture.build().unwrap();
    let expected = &plans.units()[0];
    decode(expected)
        .validate_against(expected, &mut meter())
        .unwrap();
    let changed = StrongInitializationUnitSemanticPlanV1::from_artifact(
        expected.unit(),
        expected.diagnostic_path().to_owned(),
        expected.schedule(),
        expected.storage(),
        expected.storage(),
        expected.initializer(),
        expected.ensure(),
        expected.dependencies().to_vec(),
    );
    assert!(matches!(
        decode(&changed).validate_against(expected, &mut meter()),
        Err(StrongSemanticProjectionError::Mismatch)
    ));
    for limits in [
        DecodeLimits {
            validation_work_units: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decode(expected).validate_against(expected, &mut BudgetMeter::new(limits)),
            Err(StrongSemanticProjectionError::Resource(_))
        ));
    }
}
