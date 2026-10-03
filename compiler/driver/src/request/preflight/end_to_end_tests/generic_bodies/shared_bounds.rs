#[test]
fn shared_bounds_reject_conflicts_and_unconstrained_parameters() {
    super::members::check_fixture_cases(
        "m23-shared-bounds",
        &[],
        &["unconstrained", "conflicting", "invariant", "missing-bound"],
        "downstream",
    );
}
