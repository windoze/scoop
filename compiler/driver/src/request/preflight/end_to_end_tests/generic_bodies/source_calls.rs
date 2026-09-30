#[test]
fn source_calls_share_targets_defaults_and_evaluation_order() {
    super::members::check_fixture_cases(
        "m23-shared-source-calls",
        &["standalone", "combined"],
        &["bad-managed", "bad-ordinary", "bad-constraint"],
        "downstream",
    );
}
