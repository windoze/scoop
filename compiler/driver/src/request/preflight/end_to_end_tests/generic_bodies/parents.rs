#[test]
fn closed_generic_parents_survive_artifact_republication() {
    super::members::check_fixture_cases(
        "m23-shared-parents",
        &["classes", "interfaces", "combined"],
        &["bad-class-argument", "bad-interface-arity", "bad-override"],
        "downstream",
    );
}
