use super::*;

#[test]
fn defaults_compare_common_body_and_contract_without_reinterpreting_reference_sets() {
    let new = new_default(false, CanonicalBooleanV1::False);
    assert!(super::super::defaults::equal(&new, &old_default(false), &path()).unwrap());
    assert!(!super::super::defaults::equal(&new, &old_default(true), &path()).unwrap());
    assert!(
        !super::super::defaults::equal(
            &new_default(false, CanonicalBooleanV1::True),
            &old_default(false),
            &path()
        )
        .unwrap()
    );
}
