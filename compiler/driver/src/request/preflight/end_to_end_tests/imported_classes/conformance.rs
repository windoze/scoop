use super::*;

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
