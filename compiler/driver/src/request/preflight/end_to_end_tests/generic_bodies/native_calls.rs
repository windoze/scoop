#[test]
fn ordinary_calls_share_argument_checks_after_republication() {
    super::members::check_fixture_cases(
        "m23-shared-native-calls",
        &["context", "variance"],
        &[
            "bad-result-free",
            "bad-result-local",
            "bad-result-member",
            "bad-result-local-member",
            "bad-result-extension",
            "bad-argument",
            "bad-spread",
        ],
        "downstream",
    );
}
