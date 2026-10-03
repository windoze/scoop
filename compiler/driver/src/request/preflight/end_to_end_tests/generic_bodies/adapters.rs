#[test]
fn delegate_initializer_adapters_republish_and_execute() {
    super::members::check_fixture_cases(
        "m23-function-adapters",
        &["delegate-static", "delegate-dynamic"],
        &[],
        "downstream",
    );
}

#[test]
fn adapted_function_failures_preserve_catch_and_finally() {
    super::members::check_fixture_cases("m23-function-adapters", &["failures"], &[], "downstream");
}

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
