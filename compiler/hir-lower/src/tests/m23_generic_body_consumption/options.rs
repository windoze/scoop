use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-options")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_option_roles_and_generic_variants_use_actual_declarations() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("construct"),
        |output, _, _, _, _| {
            let export = output.output().export.module();
            assert!(
                export.enums.is_empty(),
                "imported enum declarations stay in their provider"
            );
            let local = output.output().local.module();
            let options = local
                .enums
                .iter()
                .filter(|(_, enumeration)| enumeration.name == "Option")
                .collect::<Vec<_>>();
            assert!(!options.is_empty());
            for (id, enumeration) in options {
                let roles = local
                    .option_core(id)
                    .expect("the concrete application retains the imported roles");
                assert_eq!(
                    enumeration.variants[roles.some().variant().into_raw() as usize].fields[0].ty,
                    enumeration.type_arguments[0]
                );
            }
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let dependencies =
                scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new())
                    .unwrap();
            scoop_mir_lower::lower_current_cone(&output, dependencies).unwrap();
        },
    )
    .unwrap();
}

#[test]
fn imported_option_source_combinations_lower_before_publication() {
    for case in [
        "qualified",
        "sugar",
        "values",
        "static",
        "defaults",
        "lexical",
        "shadow",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
