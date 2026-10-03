#[test]
fn imported_pointer_construction_templates_accept_downstream_pointees() {
    super::members::check_fixture_cases(
        "m23-imported-pointer-construction",
        &["templates"],
        &[],
        "templates-downstream",
    );
}
