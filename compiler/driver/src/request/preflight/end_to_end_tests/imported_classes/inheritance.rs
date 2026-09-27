use super::*;

#[test]
fn inherited_class_abi_and_initialization_run_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "inheritance-abi-zst",
            "inheritance-abi-wide",
            "inheritance-abi-reference",
            "inheritance-abi-secondary",
            "inheritance-abi-singleton",
        ],
        &[
            "inheritance-abi-escape",
            "inheritance-abi-computed-init",
            "inheritance-abi-private-init-setter",
        ],
    );
}

#[test]
fn inherited_class_access_uses_actual_dependency_declarations() {
    check_class_cases(
        "direct",
        &[
            "inheritance-access-method",
            "inheritance-access-property",
            "inheritance-access-nested",
            "inheritance-access-values",
        ],
        &[],
    );
}

#[test]
fn inherited_class_access_reports_language_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "inheritance-access-base-receiver",
            "inheritance-access-sibling-receiver",
            "inheritance-access-nested-outside",
            "inheritance-access-nested-exposure",
            "inheritance-access-setter",
            "inheritance-access-narrow-override",
        ],
    );
}

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
