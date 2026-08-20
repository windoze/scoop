use super::*;

#[test]
fn ordinary_classes_keep_closed_generic_parents() {
    check_fixture("classes");
}

#[test]
fn ordinary_interfaces_keep_closed_generic_parents() {
    check_fixture("interfaces");
}

#[test]
fn closed_parents_compose_with_dispatch_and_nested_applications() {
    check_fixture("combined");
}

fn check_fixture(name: &str) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-parents");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| {})
        .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
}
