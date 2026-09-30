#[test]
fn constructor_requests_reuse_original_applications_without_emitting_lexical_parents() {
    super::members::check_fixture_cases(
        "m23-shared-constructor-requests",
        &["standalone", "combined", "both", "both-imported"],
        &["bad-arity"],
        "downstream",
    );
}
