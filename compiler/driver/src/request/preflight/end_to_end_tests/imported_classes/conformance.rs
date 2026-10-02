use super::*;

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
