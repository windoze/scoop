#[test]
fn implicit_receiver_properties_republish_from_both_declaration_locations() {
    super::members::check_fixture_cases(
        "m23-shared-host-properties",
        &["standalone", "combined"],
        &["bad-private-local", "bad-private-imported"],
        "downstream",
    );
}
