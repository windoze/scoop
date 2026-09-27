use super::*;

#[test]
fn local_types_implement_dependency_interfaces_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "conformance-class",
            "conformance-struct",
            "conformance-enum",
            "conformance-default",
            "conformance-property",
            "conformance-property-readonly",
            "conformance-abi",
            "conformance-conflict-resolved",
        ],
        &[],
    );
}

#[test]
fn local_types_implement_dependency_interfaces_report_source_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "conformance-missing",
            "conformance-override-missing",
            "conformance-override-signature",
            "conformance-override-effect",
            "conformance-override-visibility",
            "conformance-default-conflict",
            "conformance-property-readonly-error",
            "conformance-property-type-error",
            "conformance-property-value-mutable",
            "conformance-property-override-missing",
        ],
    );
}
