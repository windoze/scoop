#[test]
fn generic_bound_constraints_reject_invalid_calls_and_declarations() {
    super::members::check_fixture_cases(
        "m23-generic-bounds",
        &[],
        &[
            "bad-interface-bound",
            "bad-combined-bound",
            "bad-class-bound",
            "bad-exact-interface-bound",
            "bad-reference-result",
            "bad-default-bound",
            "bad-default-no-gc",
            "bad-source-duplicate-interface",
            "bad-source-mixed-classes",
            "bad-source-imported-classes",
            "bad-source-kind-bound",
        ],
        "downstream",
    );
}

#[test]
fn generic_bound_primitives_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases("m23-generic-bounds", &["primitives"], &[], "downstream");
}

#[test]
fn generic_bound_values_merge_with_downstream_box_dispatch() {
    super::members::check_fixture_cases("m23-generic-bounds", &["value"], &[], "downstream-value");
}
