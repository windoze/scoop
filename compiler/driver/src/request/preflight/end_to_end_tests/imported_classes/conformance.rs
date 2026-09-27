use super::*;

#[test]
fn inherited_dependency_parameters_preserve_defaults_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "conformance-parameter-simple",
            "conformance-parameter-dynamic",
            "conformance-parameter-diamond",
            "conformance-parameter-interface",
            "conformance-parameter-zst",
            "conformance-parameter-abi",
            "conformance-parameter-named",
            "conformance-parameter-published",
        ],
        &[],
    );
}

#[test]
fn inherited_dependency_parameters_report_source_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "conformance-parameter-conflict",
            "conformance-parameter-redeclared",
            "conformance-parameter-wrong-name",
            "conformance-parameter-wrong-type",
        ],
    );
}

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

#[test]
fn local_interfaces_inherit_dependency_members_through_actual_artifacts() {
    check_class_cases(
        "direct",
        &[
            "conformance-interface",
            "conformance-interface-diamond",
            "conformance-interface-property",
            "conformance-interface-overload",
        ],
        &[],
    );
}

#[test]
fn local_interfaces_inherit_dependency_members_report_source_errors() {
    check_class_cases(
        "direct",
        &[],
        &[
            "conformance-interface-override-missing",
            "conformance-interface-override-effect",
            "conformance-interface-override-visibility",
            "conformance-interface-conflict",
            "conformance-interface-cycle",
        ],
    );
}

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
