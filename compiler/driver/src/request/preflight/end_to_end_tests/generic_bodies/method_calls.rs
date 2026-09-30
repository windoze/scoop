#[test]
fn method_calls_share_bound_targets_defaults_and_dispatch() {
    super::members::check_fixture_cases(
        "m23-shared-method-calls",
        &["standalone", "combined"],
        &[
            "bad-managed",
            "bad-class-bound",
            "bad-interface-bound",
            "bad-narrow",
        ],
        "downstream",
    );
}
