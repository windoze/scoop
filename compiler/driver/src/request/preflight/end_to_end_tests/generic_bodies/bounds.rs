#[test]
fn generic_bound_primitives_republish_and_execute_from_artifacts() {
    super::members::check_fixture_cases("m23-generic-bounds", &["primitives"], &[], "downstream");
}

#[test]
fn generic_bound_values_merge_with_downstream_box_dispatch() {
    super::members::check_fixture_cases("m23-generic-bounds", &["value"], &[], "downstream-value");
}
