#[test]
fn private_function_signature_types_survive_tuple_boxing_and_republication() {
    super::members::check_fixture_cases(
        "m23-function-adapters",
        &["private-signature"],
        &[],
        "downstream",
    );
}
