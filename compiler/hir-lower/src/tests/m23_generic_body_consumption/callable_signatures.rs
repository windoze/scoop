use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-callable-signatures")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_and_dependency_functions_retain_complete_shared_signatures() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                let source = export
                    .functions
                    .values()
                    .find(|function| function.name == "localIdentity")
                    .unwrap();
                let imported = |name: &str| {
                    &export
                        .imported_generic_templates
                        .values()
                        .find(|function| function.name == name)
                        .unwrap()
                        .signature
                        .signature
                };
                for signature in [&source.signature, imported("identity")] {
                    assert_eq!(signature.params.len(), 1);
                    assert_eq!(signature.params[0].name, "value");
                    assert_eq!(signature.return_ty, signature.params[0].ty);
                    assert!(matches!(
                        export.types[signature.return_ty],
                        hir::Type::Param(_)
                    ));
                    assert!(!signature.is_suspend);
                    assert_eq!(signature.attributes, hir::FunctionAttributes::default());
                }
                assert_eq!(imported("scalar").attributes.gc_effect, hir::GcEffect::NoGc);
                assert_eq!(imported("unchecked").attributes.safety, hir::Safety::Unsafe);
                assert!(imported("delayed").is_suspend);
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn decoded_callable_effects_reject_invalid_calling_contexts() {
    for (case, message) in [
        ("bad-nogc", "@NoGC"),
        ("bad-unsafe", "unsafe"),
        ("bad-suspend", "suspend"),
        ("bad-suspend-member", "suspend"),
        ("bad-suspend-default", "suspend"),
    ] {
        let errors =
            with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| ())
                .expect_err(case);
        assert!(
            errors
                .iter()
                .all(|error| error.file == 0 && error.span.is_some())
        );
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{case}: {errors:?}"
        );
    }
}
