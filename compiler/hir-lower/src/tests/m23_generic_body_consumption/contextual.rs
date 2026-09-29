use super::*;

#[test]
fn source_and_loaded_arguments_share_contextual_fixed_points() {
    check_fixture("shared-fixed-point");
}

#[test]
fn source_and_loaded_parameter_protocols_share_the_mapper() {
    check_fixture("shared-mapping");
}

fn check_fixture(name: &str) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-argument-inference");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| {})
        .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
}
