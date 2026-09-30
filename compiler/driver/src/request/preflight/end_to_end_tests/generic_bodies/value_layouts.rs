#[test]
fn dependency_value_wrappers_preserve_inline_layout_boundaries() {
    super::members::check_fixture_cases(
        "m23-shared-value-layouts",
        &["standalone", "combined"],
        &["struct-cycle", "enum-cycle", "mixed-cycle", "growing-cycle"],
        "downstream",
    );
}
