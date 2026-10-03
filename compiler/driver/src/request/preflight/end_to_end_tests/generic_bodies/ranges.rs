#[test]
fn selected_interface_members_preserve_overloads_and_defaults() {
    super::members::check_fixture_cases(
        "m23-shared-interface-members",
        &["standalone", "combined"],
        &["bad-overload"],
        "downstream",
    );
}
