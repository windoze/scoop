use super::*;

#[test]
fn inherited_classes_compile_and_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "inheritance-constructor",
            "inheritance-override",
            "inheritance-super",
            "inheritance-default",
            "inheritance-interface",
            "inheritance-property",
            "inheritance-core",
            "inheritance-abstract",
            "inheritance-protected",
            "inheritance-published",
        ],
        &[],
    );
}

#[test]
fn inherited_classes_report_source_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "inheritance-final",
            "inheritance-missing-override",
            "inheritance-no-override-target",
            "inheritance-final-method",
            "inheritance-abstract-obligation",
            "inheritance-private-constructor",
        ],
    );
}
