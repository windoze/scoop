#[test]
fn generic_bound_values_merge_with_downstream_box_dispatch() {
    super::members::check_fixture_cases("m23-generic-bounds", &["value"], &[], "downstream-value");
}
