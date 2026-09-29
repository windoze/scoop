use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-source-callable-references")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_callable_references_select_dependency_targets_and_publish() {
    for case in [
        "named",
        "generic",
        "extensions",
        "primitives",
        "members",
        "protected",
        "dispatch",
        "bounds",
        "overloads",
        "layers",
        "defaults",
        "lexical",
        "values",
        "global-state",
        // Imported coroutine protocols still require a separate machine path.
        "suspend-signature",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, world, _, _, _| {
                let export = output.output().export.module();
                let local = output.output().local.module();
                assert!(
                    export
                        .callable_references
                        .values()
                        .any(|reference| matches!(
                            reference.target,
                            hir::CallableReferenceTarget::Imported(_)
                        )),
                    "{case}"
                );
                assert!(
                    export
                        .functions
                        .values()
                        .all(|function| !["identity", "retain", "select", "pack"]
                            .contains(&function.name.as_str())),
                    "foreign functions retain their dependency identities"
                );
                assert!(!local.callable_references.is_empty(), "{case}");
                if matches!(case, "overloads" | "layers") {
                    let selected = export
                        .imported_generic_templates
                        .values()
                        .filter(|template| template.name == "select")
                        .collect::<Vec<_>>();
                    assert_eq!(
                        selected.len(),
                        1,
                        "only the winning declaration commits its template"
                    );
                    assert_eq!(selected[0].type_parameters.len(), 2);
                }
                let mut foundation =
                    hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
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
                .unwrap_or_else(|error| panic!("{case}: {error:?}"));
                foundation
                    .complete_cross_cone_interface_source_points(export, &interface)
                    .unwrap();
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn source_callable_references_enforce_signature_rules_at_the_reference() {
    for (case, expected, token) in [
        (
            "bad-generic",
            "require an expected function type",
            "::identity",
        ),
        (
            "bad-owner-inference",
            "cannot infer a unique type argument",
            "::unresolved",
        ),
        ("bad-overload", "ambiguous", "::overload"),
        ("bad-ambiguous", "ambiguous", "::conflict"),
        ("bad-protected", "has no method", "read"),
        ("bad-result", "is not equal to", "::answer"),
        ("bad-default-arity", "callable shape expects", "::offset"),
        ("bad-kind", "must satisfy `value`", "::valueOnly"),
        (
            "bad-unsafe",
            "unsafe functions cannot be stored",
            "::unsafeAnswer",
        ),
        (
            "bad-suspend",
            "ordinary and suspend callable shapes differ",
            "::suspended",
        ),
        (
            "bad-generic-member",
            "require an expected function type",
            "Plain(1)::choose",
        ),
        (
            "bad-unbound-member",
            "unbound member reference",
            "Plain::read",
        ),
        ("bad-constructor", "constructor reference", "::Plain"),
    ] {
        let source = fixture(case);
        let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
            .expect_err(case);
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        assert_eq!(error.file, 0, "{case}");
        let span = error.span.unwrap();
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            token,
            "{case}"
        );
    }
}
