#[test]
fn shared_bounds_propagate_through_source_and_artifact_calls() {
    super::members::check_fixture_cases(
        "m23-shared-bounds",
        &["interfaces", "classes", "context"],
        &[],
        "downstream",
    );
}

#[test]
fn shared_bounds_reject_conflicts_and_unconstrained_parameters() {
    super::members::check_fixture_cases(
        "m23-shared-bounds",
        &[],
        &["unconstrained", "conflicting", "invariant", "missing-bound"],
        "downstream",
    );
}
