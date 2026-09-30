#[test]
fn struct_definitions_share_recursive_fields_and_copy_updates() {
    super::members::check_fixture_cases(
        "m23-shared-struct-definitions",
        &["standalone", "combined"],
        &["duplicate", "unknown", "wrong-type"],
        "downstream",
    );
}

#[test]
fn shared_struct_instances_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-structs",
        &["standalone", "combined"],
        &["bad-payload", "bad-nogc"],
        "downstream",
    );
}
