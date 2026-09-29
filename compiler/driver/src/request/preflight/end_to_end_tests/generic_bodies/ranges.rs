#[test]
fn integer_ranges_preserve_iteration_through_artifact_republication() {
    super::members::check_fixture_cases(
        "m23-shared-ranges",
        &["standalone", "combined"],
        &[],
        "downstream",
    );
}

#[test]
fn selected_interface_members_preserve_overloads_and_defaults() {
    super::members::check_fixture_cases(
        "m23-shared-interface-members",
        &["standalone", "combined"],
        &["bad-overload"],
        "downstream",
    );
}
