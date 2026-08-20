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
                            reference.target.callee(&export.bound_callable_refs),
                            Some(
                                hir::CallableTarget::Application(_)
                                    | hir::CallableTarget::Dependency(_)
                            )
                        ) || matches!(
                            reference.target,
                            hir::CallableReferenceTarget::BoundIntrinsic { .. }
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
