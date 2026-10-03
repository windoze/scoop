use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-equality")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_equality_fields_and_variants_materialize() {
    for case in [
        "payload",
        "enum",
        "nested",
        "overloads",
        "empty",
        "order",
        "defaults",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                assert!(
                    output
                        .output()
                        .local
                        .module()
                        .functions
                        .iter()
                        .any(|(_, function)| {
                            function.modifiers.operator == Some(hir::OperatorKind::Equals)
                                && matches!(function.kind, hir::concrete::FunctionKind::User(_))
                        })
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn imported_generic_equality_rejects_incomparable_fields_at_the_operator() {
    for (case, expected) in [
        (
            "bad-reference",
            "has no applicable member operator `equals`",
        ),
        ("bad-enum", "has no applicable member operator `equals`"),
        (
            "bad-extension",
            "has no applicable member operator `equals`",
        ),
        (
            "bad-static-type",
            "has no applicable member operator `equals`",
        ),
        ("bad-exact-type", "has no member operator `equals`"),
        ("bad-no-gc", "@NoGC"),
    ] {
        let source = fixture(case);
        let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
            .expect_err(case);
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        let span = error.span.unwrap();
        assert_eq!(error.file, 0, "{case}");
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "left == right",
            "{case}"
        );
    }
}

#[test]
fn imported_generic_derived_equality_materializes_complete_bodies() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("struct"),
        |output, world, _, _, _| {
            let mut foundation =
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let export = output.output().export.module();
            let local = output.output().local.module();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &export.public_export_bindings,
                world,
            );
            let interface = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &output,
                &[],
                &mut authority,
            )
            .unwrap();
            foundation
                .complete_cross_cone_interface_source_points(export, &interface)
                .unwrap();
            assert!(local.functions.iter().any(|(_, function)| {
                function.modifiers.operator == Some(hir::OperatorKind::Equals)
                    && matches!(function.kind, hir::concrete::FunctionKind::User(_))
            }));
            let mut positions = 0;
            for reference in interface.external_references().records() {
                for site in reference
                    .type_sites()
                    .records()
                    .iter()
                    .filter_map(|site| site.as_expression())
                {
                    let root = site.position().root;
                    let scoop_identity::CallableTemplateOwner::Generated(id) = root.template()
                    else {
                        continue;
                    };
                    if !local.generated_callable_identities.iter().any(|record| {
                        record.id() == id
                            && matches!(
                                record.key(),
                                scoop_identity::GeneratedCallableKey::DerivedEquality { .. }
                            )
                    }) {
                        continue;
                    }
                    let origin = site.origin().evaluation();
                    foundation
                        .validate_executable_evaluation_origin(
                            local.cone,
                            root,
                            origin,
                            &foundation,
                        )
                        .unwrap_or_else(|error| panic!("{:?}: {error:?}", site.position()));
                    let invalid = scoop_identity::EvaluationOrigin::new(
                        origin.source().clone(),
                        scoop_identity::SourceSpan::new(0, u64::MAX).unwrap(),
                        foundation.source_context_key(origin.context()).unwrap(),
                    )
                    .unwrap();
                    assert!(matches!(
                        foundation.validate_executable_evaluation_origin(
                            local.cone,
                            root,
                            &invalid,
                            &foundation
                        ),
                        Err(hir::ExecutableEvaluationValidationError::Location(_)),
                    ));
                    positions += 1;
                }
            }
            assert!(positions > 0);
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}
