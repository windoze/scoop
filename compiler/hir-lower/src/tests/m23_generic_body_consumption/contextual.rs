use super::*;

#[test]
fn source_and_loaded_arguments_share_contextual_fixed_points() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-argument-inference");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join("shared-fixed-point.scoop")).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| {})
        .expect("failed empty-array seeds must not prevent complete anonymous-function seeds");
}
