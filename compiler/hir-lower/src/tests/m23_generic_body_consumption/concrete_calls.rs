use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/language/shared-concrete-calls")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn debug_declarations_keep_current_and_imported_source_coordinates_through_mir() {
    with_provider_consumer(
        "package debug.source\n\npublic fun <T> original(value: T): T = value\n",
        "import debug.source.original\n\nfun localDebug(): Int = original<Int>(7)\n",
        |output, _, _, _, _| {
            let local = output.output().local.module();
            let mut records = scoop_source::DebugRecords::default();
            let expected = ["original", "localDebug"].map(|name| {
                let function = local
                    .functions
                    .values()
                    .find(|function| function.name == name)
                    .unwrap();
                let origin = local
                    .debug_sources
                    .function(function.materialization, &mut records);
                let scoop_source::DebugOrigin::Source(source) = &origin else {
                    panic!("source function {name} must retain its declaration");
                };
                assert_eq!((source.line, source.column), (3, 1));
                origin
            });
            assert_ne!(expected[0], expected[1]);
            let dependencies =
                scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new())
                    .unwrap();
            let mir = scoop_mir_lower::lower_current_cone(
                &output,
                dependencies,
                Default::default(),
                None,
            )
            .unwrap();
            for expected in expected {
                assert!(
                    mir.module()
                        .functions
                        .values()
                        .any(|function| function.debug_origin == expected)
                );
            }
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
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
