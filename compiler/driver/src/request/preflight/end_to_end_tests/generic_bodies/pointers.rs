#[test]
fn imported_pointer_templates_accept_downstream_pointees() {
    super::members::check_fixture_cases(
        "m23-imported-pointers",
        &["template"],
        &[],
        "template-downstream",
    );
}
