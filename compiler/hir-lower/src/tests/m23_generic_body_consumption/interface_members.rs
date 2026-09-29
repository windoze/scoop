use super::*;

#[test]
fn interface_member_selection_is_independent_of_declaration_storage() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| {})
            .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

fn fixture(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-interface-members");
    std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap()
}
