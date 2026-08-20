use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-pointers")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_pointer_cast_checks_direct_and_deferred_pointees() {
    for case in ["bad-cast-pointee", "bad-generic-cast"] {
        let errors =
            with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| ())
                .expect_err("a cast to a value containing a managed reference must fail");
        assert!(
            errors.iter().any(|error| error.message.contains("GC-free")),
            "{case}: {errors:?}"
        );
    }
}

#[test]
fn imported_pointer_intrinsics_use_source_places_and_typed_templates() {
    for case in [
        "local",
        "template",
        "operations",
        "layout",
        "this-copy",
        "zst",
        "defaults",
        "bindings",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                assert!(
                    export
                        .imported_generic_templates
                        .iter()
                        .all(
                            |(_, function)| !["addressOf", "sizeOf", "alignOf", "load", "store"]
                                .contains(&function.name.as_str())
                        )
                );
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                let local = output.output().local.module();
                let dependencies =
                    scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new())
                        .unwrap();
                scoop_mir_lower::lower_current_cone(&output, dependencies, Default::default())
                    .unwrap();
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
