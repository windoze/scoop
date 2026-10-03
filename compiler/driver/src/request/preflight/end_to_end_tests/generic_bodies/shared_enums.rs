#[test]
fn shared_enum_patterns_retain_generic_payload_diagnostics() {
    super::members::check_fixture_cases(
        "m23-shared-enums",
        &[],
        &[
            "arity",
            "named-style",
            "wrong-alias",
            "incomplete",
            "hidden-prefix",
            "wrong-prefix",
        ],
        "downstream",
    );
}
