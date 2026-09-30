#[test]
fn function_values_preserve_callee_and_argument_evaluation_order() {
    super::members::check_fixture_cases(
        "m23-shared-callable-order",
        &["standalone", "combined"],
        &[],
        "downstream",
    );
}
