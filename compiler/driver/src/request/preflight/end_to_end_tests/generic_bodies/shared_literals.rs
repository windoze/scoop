#[test]
fn shared_literal_patterns_republish_defaults_and_generic_bodies_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-literals",
        &["standalone", "combined"],
        &["bad-literal", "bad-exhaustiveness"],
        "downstream",
    );
}
