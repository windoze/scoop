#[test]
fn default_bodies_share_capture_and_substitution_rules_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-default-bodies",
        &["functions", "methods"],
        &["bad-callback", "bad-later-parameter"],
        "downstream",
    );
}
