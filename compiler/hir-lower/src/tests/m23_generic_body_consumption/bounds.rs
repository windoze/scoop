use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-bounds")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_bound_calls_and_references_materialize_selected_members() {
    for case in [
        "value",
        "virtual",
        "generic-virtual",
        "generic-interface",
        "generic-interface-default",
        "interface-receiver",
        "interface-default",
        "interface-enum",
        "interface-abstract",
        "local-conformance",
        "source-bounds",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                hir::CanonicalHirFoundation::from_dependency_output(&output)
                    .unwrap_or_else(|error| panic!("{case}: {error:?}"));
                let local = output.output().local.module();
                assert!(!local.callable_references.is_empty(), "{case}");
                assert!(
                    output
                        .output()
                        .export
                        .module()
                        .functions
                        .iter()
                        .all(|(_, function)| {
                            ![
                                "readBound",
                                "readReference",
                                "genericClassGet",
                                "inheritedReference",
                            ]
                            .contains(&function.name.as_str())
                        }),
                    "provider declarations stay outside the consumer source arena: {case}"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn imported_bound_core_primitive_calls_materialize_selected_members() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("primitives"),
        |output, _, _, _, _| {
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let local = output.output().local.module();
            assert_eq!(
                local
                    .functions
                    .iter()
                    .filter(|(_, function)| function.name == "stringBound")
                    .count(),
                10
            );
            assert_eq!(
                local
                    .functions
                    .iter()
                    .filter(|(_, function)| function.name == "hashBound")
                    .count(),
                1
            );
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn imported_bound_defaults_preserve_substituted_receivers_when_republished() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("defaults"),
        |output, world, _, _, _| {
            let mut foundation =
                hir::CanonicalHirFoundation::from_type_semantics_output(&output).unwrap();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &output.output().export.public_export_bindings,
                world,
            );
            let interface = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &output,
                &[],
                &mut authority,
            )
            .unwrap();
            for template in interface.default_templates().records() {
                template
                    .validate_reference_closure(&scoop_wire::WirePath::root())
                    .unwrap();
            }
            foundation
                .complete_cross_cone_interface_source_points(
                    output.output().export.module(),
                    &interface,
                )
                .unwrap();
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn imported_bound_default_effect_errors_point_to_the_consuming_call() {
    let source = fixture("bad-default-no-gc");
    let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
        .expect_err("managed bound defaults cannot execute in @NoGC code");
    assert!(!errors.is_empty());
    for error in errors {
        let span = error.span.unwrap();
        assert_eq!(error.file, 0);
        assert_eq!(
            source.get(span.start as usize..span.end as usize),
            Some("defaultRead(Value(42))"),
            "{error:?}"
        );
        assert!(error.message.contains("@NoGC"), "{error:?}");
    }
}
