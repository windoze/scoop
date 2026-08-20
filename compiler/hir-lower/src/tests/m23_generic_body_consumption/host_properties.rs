use super::*;

#[test]
fn implicit_receiver_properties_do_not_require_a_source_host() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-host-properties");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join("combined.scoop")).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| ())
        .unwrap_or_else(|errors| panic!("{errors:?}"));
}
