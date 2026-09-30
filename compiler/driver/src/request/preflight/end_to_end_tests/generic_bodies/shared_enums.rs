#[test]
fn enum_definitions_reuse_binders_for_recursive_payloads_and_interfaces() {
    super::members::check_fixture_cases(
        "m23-shared-enum-definitions",
        &["standalone", "combined"],
        &[],
        "downstream",
    );
}

#[test]
fn shared_enum_references_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-enums",
        &[
            "standalone",
            "defaults-nested",
            "aliases-equality",
            "qualified",
            "instances",
        ],
        &[],
        "downstream",
    );
}

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
