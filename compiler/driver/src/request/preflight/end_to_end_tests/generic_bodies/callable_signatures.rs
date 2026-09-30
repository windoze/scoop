#[test]
fn shared_callable_signatures_preserve_effects_and_lexical_bodies() {
    super::members::check_fixture_cases(
        "m23-shared-callable-signatures",
        &["standalone", "combined"],
        &[
            "bad-nogc",
            "bad-unsafe",
            "bad-suspend",
            "bad-suspend-member",
            "bad-suspend-default",
        ],
        "downstream",
    );
}
