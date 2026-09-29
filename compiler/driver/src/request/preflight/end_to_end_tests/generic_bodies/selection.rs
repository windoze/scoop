#[test]
fn shared_specificity_survives_artifact_republication() {
    super::members::check_fixture_cases(
        "m23-shared-selection",
        &["functions", "members", "constructors"],
        &[
            "bad-free",
            "bad-local",
            "bad-member",
            "bad-local-member",
            "bad-constructor",
            "bad-local-constructor",
        ],
        "downstream",
    );
}
