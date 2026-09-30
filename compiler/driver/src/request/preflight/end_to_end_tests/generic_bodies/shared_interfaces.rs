#[test]
fn shared_interface_instances_preserve_inherited_substitutions_through_artifacts() {
    super::members::check_fixture_cases(
        "m23-shared-interfaces",
        &["standalone", "combined"],
        &["bad-missing", "bad-result"],
        "downstream",
    );
}

#[test]
fn interface_definitions_share_recursive_signatures_and_composed_defaults() {
    super::members::check_fixture_cases(
        "m23-shared-interface-definitions",
        &["standalone", "combined"],
        &[
            "bad-invariant",
            "bad-override",
            "bad-missing",
            "bad-recursive",
        ],
        "downstream",
    );
}
