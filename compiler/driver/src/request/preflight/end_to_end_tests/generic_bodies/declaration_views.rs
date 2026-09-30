#[test]
fn declaration_views_share_calls_and_reference_applicability() {
    super::members::check_fixture_cases(
        "m23-shared-declaration-views",
        &["standalone", "combined"],
        &[
            "bad-default-arity-local",
            "bad-default-arity-imported",
            "bad-vararg-shape-local",
            "bad-vararg-shape-imported",
            "bad-generic-expected-local",
            "bad-generic-expected-imported",
        ],
        "downstream",
    );
}
