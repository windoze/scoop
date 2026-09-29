use super::*;

#[test]
fn callable_and_nominal_candidates_share_specificity() {
    check_fixture("functions");
}

#[test]
fn source_and_dependency_members_share_specificity() {
    check_fixture("members");
}

#[test]
fn constructor_calls_and_delegations_share_specificity() {
    check_fixture("constructors");
}

fn check_fixture(name: &str) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-selection");
    let provider = std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap();
    let consumer = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    with_provider_consumer(&provider, &consumer, |_, _, _, _, _| {})
        .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
}
