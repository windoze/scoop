use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-pointer-construction")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_pointer_construction_uses_bindings_and_preserves_templates() {
    for case in [
        "local",
        "aliases",
        "templates",
        "defaults",
        "lexical",
        "overloads",
        "zst-nested",
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

#[test]
fn imported_pointer_construction_preserves_nonzero_and_pointee_requirements() {
    for (case, expected) in [
        ("bad-zero", "nonzero"),
        ("bad-folded-zero", "nonzero"),
        ("bad-zero-overload", "nonzero"),
        ("bad-unsafe-overload", "unsafe"),
        ("bad-managed-value", "GC-free"),
        ("bad-generic-pointee", "GC-free"),
        ("bad-default-pointee", "GC-free"),
        ("bad-ambiguous", "ambiguous"),
        ("bad-fixed-ambiguous", "ambiguous"),
    ] {
        let errors =
            with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| ())
                .expect_err("the source violates a pointer construction requirement");
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{case}: {errors:?}"
        );
    }
}
