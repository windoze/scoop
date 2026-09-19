use super::*;

#[test]
fn defaults_compare_common_body_and_contract_without_reinterpreting_reference_sets() {
    let new = new_default(false, CanonicalBooleanV1::False);
    assert!(
        super::super::defaults::equal(&new, &old_default(false), &mut meter(), &path()).unwrap()
    );
    assert!(
        !super::super::defaults::equal(&new, &old_default(true), &mut meter(), &path()).unwrap()
    );
    assert!(
        !super::super::defaults::equal(
            &new_default(false, CanonicalBooleanV1::True),
            &old_default(false),
            &mut meter(),
            &path()
        )
        .unwrap()
    );
}

#[test]
fn common_default_comparison_keeps_one_accumulating_resource_meter() {
    let new = new_default(false, CanonicalBooleanV1::False);
    let old = old_default(false);
    let mut baseline = meter();
    assert!(super::super::defaults::equal(&new, &old, &mut baseline, &path()).unwrap());
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: baseline.usage().validation_work_units * 2 - 1,
        ..DecodeLimits::default()
    });
    assert!(super::super::defaults::equal(&new, &old, &mut shared, &path()).unwrap());
    limit(
        super::super::defaults::equal(&new, &old, &mut shared, &path()).unwrap_err(),
        ResourceKind::ValidationWorkUnits,
    );
}
