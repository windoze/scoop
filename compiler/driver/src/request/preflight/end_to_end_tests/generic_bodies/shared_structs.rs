#[test]
fn shared_struct_instances_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-structs",
        &["standalone", "combined"],
        &["bad-payload", "bad-nogc"],
        "downstream",
    );
}
