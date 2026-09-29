#[test]
fn imported_arrays_and_varargs_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-generic-arrays",
        &["basic", "order", "abi", "bounds", "iteration"],
        &[
            "spread-non-array",
            "spread-mutable",
            "whole-element",
            "whole-and-elements",
            "index-type",
            "write-immutable",
            "uninferrable-empty",
            "function-value-elements",
            "spread-invariant",
        ],
        "downstream",
    );
}
