#[test]
fn shared_interface_instances_preserve_inherited_substitutions_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-interfaces",
        &["standalone", "combined"],
        &["bad-missing", "bad-result"],
        "downstream",
    );
}
