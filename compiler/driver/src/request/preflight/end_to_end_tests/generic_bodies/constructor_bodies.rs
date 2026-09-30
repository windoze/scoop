#[test]
fn constructor_bodies_preserve_initialization_order_and_captures_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-constructor-bodies",
        &["standalone", "combined"],
        &[
            "bad-read-before-init",
            "bad-terminal-field",
            "bad-initializer-return",
            "bad-lambda-return",
            "bad-capture-this",
        ],
        "downstream",
    );
}
