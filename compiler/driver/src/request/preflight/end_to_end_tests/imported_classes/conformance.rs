use super::*;

#[test]
fn abstract_dependency_conformances_keep_actual_targets_through_artifacts() {
    check_class_cases(
        "direct",
        &[
            "conformance-abstract-method",
            "conformance-abstract-default",
            "conformance-abstract-interface",
            "conformance-abstract-property",
            "conformance-abstract-abi",
        ],
        &[],
    );
}

#[test]
fn abstract_dependency_conformances_report_unimplemented_source_members() {
    check_class_cases(
        "direct",
        &[],
        &[
            "conformance-abstract-class-incomplete",
            "conformance-abstract-interface-incomplete",
            "conformance-abstract-property-incomplete",
        ],
    );
}

#[test]
fn qualified_interface_super_calls_use_actual_dependency_defaults() {
    check_class_cases(
        "direct",
        &[
            "conformance-super-method",
            "conformance-super-interface",
            "conformance-super-inherited",
            "conformance-super-property",
            "conformance-super-zst",
            "conformance-super-abi",
            "conformance-super-arguments",
            "conformance-super-default",
            "conformance-super-imported-default",
        ],
        &[],
    );
}

#[test]
fn qualified_interface_super_calls_report_source_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "conformance-super-abstract-method",
            "conformance-super-abstract-getter",
            "conformance-super-abstract-setter",
            "conformance-super-indirect",
            "conformance-super-wrong-argument",
            "conformance-super-initialization",
            "conformance-super-class-qualifier",
            "conformance-super-local-abstract",
        ],
    );
}
