#[test]
fn shared_class_instances_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-classes",
        &["standalone", "combined"],
        &["bad-payload", "bad-final-base"],
        "downstream",
    );
}
