use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-qualified-types")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn qualified_dependency_types_share_ordinary_type_resolution() {
    for case in ["ordinary", "aliases", "nested", "bounds", "split-package"] {
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

#[test]
fn qualified_dependency_types_reject_invalid_paths_and_arguments() {
    for (case, expected) in [
        ("bad-alias-arguments", "not generic"),
        ("bad-arity", "type argument"),
        ("bad-missing-arguments", "type argument"),
        ("bad-nongeneric", "not generic"),
        ("bad-hidden", "no accessible type"),
        ("bad-package", "not a type"),
        ("bad-current-longer", "not a type"),
        ("bad-conflict", "ambiguous"),
        ("bad-nested-arity", "type argument"),
        ("bad-nested-missing", "nested type"),
        ("bad-nested-private", "accessible"),
        ("bad-nested-bound", "must satisfy `value`"),
        ("bad-namespace-alias-cycle", "First -> Second -> First"),
    ] {
        let errors =
            with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| ())
                .unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{case}: {errors:?}"
        );
    }
}
