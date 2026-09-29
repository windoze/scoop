#[test]
fn original_function_requests_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-requests",
        &["standalone", "methods-closures", "captured-parameters"],
        &[],
        "downstream",
    );
}
