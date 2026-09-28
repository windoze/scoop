use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-nominal-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_nominals_substitute_payloads_and_preserve_origin() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                assert!(
                    export.enums.is_empty(),
                    "dependency templates are not local declarations"
                );
                let local = output.output().local.module();
                let parcels = local
                    .enums
                    .iter()
                    .filter(|(_, value)| value.name == "Parcel")
                    .collect::<Vec<_>>();
                assert_eq!(parcels.len(), if case == "standalone" { 1 } else { 4 });
                for (_, value) in parcels {
                    assert_eq!(value.type_arguments.len(), 1);
                    assert_eq!(
                        value.origin.source().unwrap().declaration().origin(),
                        ConeCoordinate::new("test", "generic-provider", "1.0.0")
                            .unwrap()
                            .identity()
                            .unwrap()
                    );
                    assert!(
                        local
                            .exact_type_identities
                            .nominal_specialization(value.canonical_type)
                            .is_some()
                    );
                    let item = value
                        .variants
                        .iter()
                        .find(|variant| variant.name == "Item")
                        .unwrap();
                    assert_eq!(item.fields[0].ty, value.type_arguments[0]);
                }
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                let dependencies =
                    scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new())
                        .unwrap();
                scoop_mir_lower::lower_current_cone(&output, dependencies).unwrap();
            },
        )
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}

#[test]
fn imported_generic_nominal_arguments_obey_invariance_and_bounds() {
    for (case, message, expression) in [
        ("bad-ref", "must satisfy `ref`", "Int"),
        ("bad-value", "must satisfy `value`", "Number"),
        ("bad-arity", "takes 1 type argument(s)", "Parcel"),
        ("bad-invariance", "is invariant", "actual"),
    ] {
        let source = fixture(case);
        let errors =
            with_provider_consumer(&fixture("provider"), &source, |output, _, _, _, _| output)
                .err()
                .expect("invalid nominal arguments must be rejected");
        let error = errors
            .iter()
            .find(|error| error.message.contains(message))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        assert_eq!(error.file, 0);
        let span = error.span.unwrap();
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            expression,
            "{case}: {errors:?}"
        );
    }
}
