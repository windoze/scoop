#[test]
fn class_definitions_share_recursive_fields_and_generic_base_initialization() {
    super::members::check_fixture_cases(
        "m23-shared-class-definitions",
        &["standalone", "combined"],
        &["bad-final-base", "bad-field-type", "bad-base-field"],
        "downstream",
    );
}
