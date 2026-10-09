use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-constructor-requests")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn constructor_requests_preserve_complete_owners_without_emitting_unused_parents() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(&fixture("provider"), &fixture(case), |output, _, _, _, _| {
            let module = output.output().local.module();
            let mut materializations = std::collections::HashSet::new();
            let mut shells = std::collections::HashSet::new();
            for constructor in module.class_constructors.values() {
                assert!(materializations.insert(constructor.materialization));
                let class = &module.classes[constructor.class];
                if class.name == "Shell" || class.name.starts_with("Shell<") {
                    assert_eq!(constructor.parameters.len(), 2, "the one-argument lexical parent is unused");
                    assert_eq!(class.type_arguments.len(), 2, "the phantom owner argument remains");
                    assert!(shells.insert(constructor.class), "repeated and aliased requests share one constructor");
                }
            }
            assert_eq!(shells.len(), 2, "Unit and String owner arguments remain distinct");
            for constructor in module.struct_constructors.values() {
                assert!(materializations.insert(constructor.materialization));
            }
            assert!(module.functions.values().any(|function| function.name.ends_with(".read") || function.name == "read"));
        }).unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn shared_constructor_initialization_lowers_captures_in_each_executable_body() {
    for case in ["both", "both-imported"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let dependencies =
                    scoop_mir::SelectedExternalMirSet::try_from_callables(module.cone, Vec::new())
                        .unwrap();
                scoop_mir_lower::lower_current_cone(&output, dependencies, Default::default())
                    .expect("each shared initializer retains its actual capture expressions");
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
