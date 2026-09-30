#[test]
fn call_candidates_share_constraints_and_failed_transactions() {
    super::members::check_fixture_cases(
        "m23-shared-call-probes",
        &["standalone", "combined"],
        &[
            "bad-owner",
            "bad-unfilled",
            "bad-method-arity",
            "bad-postponed",
        ],
        "downstream",
    );
}
