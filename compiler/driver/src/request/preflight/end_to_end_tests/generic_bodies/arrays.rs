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

#[test]
fn imported_array_constructors_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases(
        "m23-array-construction",
        &["basic", "aliases", "overloads", "abi", "shadow"],
        &[
            "same-immutable",
            "same-mutable",
            "wrong-element",
            "invariant-reference",
            "non-array",
            "wrong-name",
            "missing-source",
            "extra-source",
            "duplicate-source",
            "spread-source",
            "wrong-arity",
            "alias-type-arguments",
            "uninferrable-empty",
            "expected-conflict",
            "ambiguous-generic",
            "ambiguous-fixed",
        ],
        "downstream",
    );
}
