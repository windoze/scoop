#[test]
fn callable_reference_targets_share_local_definitions_and_creation_captures() {
    super::members::check_fixture_cases(
        "m23-shared-reference-targets",
        &["standalone", "combined"],
        &[
            "bad-signature",
            "bad-local-capture",
            "bad-private-default",
            "bad-bound-result",
        ],
        "downstream",
    );
}
