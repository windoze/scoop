#[test]
fn libraries_without_initialization_do_not_require_a_unit_result_record() {
    super::members::check_fixture_cases(
        "m23-demanded-initialization",
        &["standalone", "combined"],
        &[],
        "downstream",
    );
}
