use super::*;

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
