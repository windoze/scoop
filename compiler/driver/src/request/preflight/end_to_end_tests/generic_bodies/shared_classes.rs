#[test]
fn shared_class_instances_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-classes",
        &["standalone", "combined"],
        &["bad-payload", "bad-final-base"],
        "downstream",
    );
}

#[test]
fn class_definitions_share_recursive_fields_and_generic_base_initialization() {
    super::members::check_fixture_cases(
        "m23-shared-class-definitions",
        &["standalone", "combined"],
        &["bad-final-base", "bad-field-type", "bad-base-field"],
        "downstream",
    );
}
