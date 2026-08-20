use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-concrete-calls")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn common_calls_reuse_applications_without_reclassifying_current_templates() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let local = output.output().local.module();
                for name in ["identity", "localIdentity"] {
                    let applications = local
                        .functions
                        .values()
                        .filter(|function| function.name == name)
                        .map(|function| function.materialization)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        applications.len(),
                        3,
                        "{case}: {name} retains Int, String and Unit"
                    );
                    assert_eq!(
                        applications
                            .into_iter()
                            .collect::<std::collections::HashSet<_>>()
                            .len(),
                        3,
                        "aliases reuse the same complete application"
                    );
                }
                let current = local
                    .functions
                    .values()
                    .find(|function| function.name == "localIdentity")
                    .unwrap()
                    .materialization
                    .template();
                let scoop_identity::CallableTemplateOwner::GenericFunction(current) = current
                else {
                    panic!("the current identity is a generic source declaration");
                };
                let occurrences = output.committed_dependency_call_occurrences().unwrap();
                let applications = occurrences
                    .iter()
                    .filter(|occurrence| {
                        matches!(
                            occurrence.target(),
                            hir::CommittedDependencyCallTarget::Application(_)
                        )
                    })
                    .collect::<Vec<_>>();
                assert!(
                    applications
                        .iter()
                        .any(|occurrence| occurrence.binding().is_some())
                );
                assert!(
                    applications
                        .iter()
                        .any(|occurrence| occurrence.binding().is_none())
                );
                assert!(
                    applications.iter().all(|occurrence| {
                        occurrence.declaration()
                            != scoop_identity::CallableTemplateOrigin::GenericFunction(current)
                    }),
                    "current generic calls do not become dependency uses"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
