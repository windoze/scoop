#[test]
fn abstract_methods_and_accessors_preserve_dispatch_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-abstract",
        &["standalone", "combined"],
        &["bad-instantiation", "bad-implementation"],
        "downstream",
    );
}
