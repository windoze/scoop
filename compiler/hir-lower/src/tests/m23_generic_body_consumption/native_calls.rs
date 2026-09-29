use super::*;

#[test]
fn ordinary_dependency_calls_share_contextual_arguments() {
    check_fixture("context");
}

#[test]
fn ordinary_dependency_calls_preserve_callable_variance() {
    check_fixture("variance");
}

fn check_fixture(name: &str) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-native-calls");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| {})
        .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
}
