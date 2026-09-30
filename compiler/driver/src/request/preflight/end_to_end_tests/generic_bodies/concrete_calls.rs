#[test]
fn concrete_calls_share_targets_and_preserve_original_dependency_uses() {
    super::members::check_fixture_cases(
        "m23-shared-concrete-calls",
        &["standalone", "combined"],
        &[],
        "downstream",
    );
}
