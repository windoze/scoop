#[test]
fn dynamic_function_variance_keeps_interface_ancestry() {
    super::members::check_fixture_cases(
        "m23-function-adapters",
        &["interface-dynamic"],
        &[],
        "downstream",
    );
}

#[test]
fn private_function_signature_types_survive_tuple_boxing_and_republication() {
    super::members::check_fixture_cases(
        "m23-function-adapters",
        &["private-signature"],
        &[],
        "downstream",
    );
}
