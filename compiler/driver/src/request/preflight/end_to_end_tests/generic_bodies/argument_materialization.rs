#[test]
fn arguments_materialize_once_in_source_order_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-argument-materialization",
        &["standalone", "combined"],
        &["bad-vararg", "bad-duplicate", "bad-constructor", "bad-role"],
        "downstream",
    );
}
