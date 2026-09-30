#[test]
fn class_bound_properties_republish_with_original_storage_and_accessors() {
    super::members::check_fixture_cases(
        "m23-shared-bound-properties",
        &["standalone", "combined"],
        &["bad-value", "bad-immutable", "bad-member"],
        "downstream",
    );
}
