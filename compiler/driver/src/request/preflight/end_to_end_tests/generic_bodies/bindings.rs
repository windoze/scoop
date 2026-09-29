#[test]
fn shared_binding_lowering_republishes_and_executes_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-bindings",
        &[
            "structs",
            "components",
            "lambda-for",
            "defaults-when",
            "abi",
        ],
        &[],
        "downstream",
    );
}

#[test]
fn shared_binding_rejections_have_source_diagnostics() {
    super::members::check_fixture_cases(
        "m23-shared-bindings",
        &[],
        &[
            "arity",
            "unknown-field",
            "duplicate-field",
            "missing-rest",
            "wrong-owner",
            "refutable",
            "immutable",
            "class-rest",
            "missing-operator",
            "private-component",
            "incomplete-when",
        ],
        "downstream",
    );
}
