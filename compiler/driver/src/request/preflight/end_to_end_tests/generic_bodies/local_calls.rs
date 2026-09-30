#[test]
fn local_calls_share_complete_arguments_and_recursive_captures_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-local-calls",
        &["standalone", "combined"],
        &["bad-name", "bad-missing", "bad-type"],
        "downstream",
    );
}
