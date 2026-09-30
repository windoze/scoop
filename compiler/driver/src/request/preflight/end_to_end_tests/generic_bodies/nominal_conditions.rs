#[test]
fn nominal_conditions_survive_generic_bodies_and_republication() {
    super::members::check_fixture_cases(
        "m23-shared-nominal-conditions",
        &["standalone", "combined"],
        &[
            "bad-struct",
            "bad-enum",
            "bad-pointer",
            "bad-class",
            "bad-interface",
            "bad-nested",
            "bad-body",
            "bad-local-body",
            "bad-wrapper",
            "bad-carrier",
            "bad-alias",
            "bad-pointer-alias",
            "bad-enum-body",
            "bad-root-class",
            "bad-root-enum",
            "bad-default",
        ],
        "downstream",
    );
}
