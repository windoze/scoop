#[test]
fn constructor_applications_share_targets_and_complete_owners_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-constructor-applications",
        &["standalone", "combined"],
        &[
            "bad-local-managed",
            "bad-dependency-managed",
            "bad-arity",
            "bad-payload",
        ],
        "downstream",
    );
}
