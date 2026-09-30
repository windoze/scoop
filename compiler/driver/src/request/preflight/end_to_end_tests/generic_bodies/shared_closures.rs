#[test]
fn closures_share_bodies_arguments_and_captures_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-closure-bodies",
        &["standalone", "combined"],
        &[
            "bad-mutable",
            "bad-default-result",
            "bad-anonymous-result",
            "bad-parameters",
        ],
        "downstream",
    );
}
