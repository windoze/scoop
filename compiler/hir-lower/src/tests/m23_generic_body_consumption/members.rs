use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-member-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_members_keep_owner_and_method_arguments() {
    for case in [
        "standalone",
        "method-arguments",
        "signature-support",
        "value-method",
        "plain-owner",
        "inherited",
        "captured",
        "virtual",
        "abi",
        "overloads",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                assert!(export.classes.iter().all(|(_, class)| class.name != "Box"));
                assert!(
                    export
                        .imported_generic_applications
                        .values()
                        .any(|application| matches!(
                            application.arguments,
                            hir::ImportedCallableArguments::Method { .. }
                        ))
                );
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                if case == "signature-support" {
                    let names = output
                        .output()
                        .local
                        .materialization()
                        .roots()
                        .iter()
                        .map(|root| root.declaration().name())
                        .collect::<Vec<_>>();
                    assert!(names.iter().any(|name| matches!(
                        name,
                        scoop_identity::DeclarationName::Named(name)
                            if name.as_str() == "SignatureValue"
                    )));
                    assert!(!names.iter().any(|name| matches!(
                        name,
                        scoop_identity::DeclarationName::Named(name)
                            if name.as_str() == "Unrelated"
                    )));
                }
            },
        )
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}

#[test]
fn imported_generic_members_enforce_both_parameter_groups() {
    for (case, expected, token) in [
        ("ambiguous-overload", "ambiguous", "conflict("),
        ("bad-method-kind", "must satisfy `value`", "Reference>("),
        (
            "bad-method-arity",
            "expects 1 type argument(s), found 2",
            "choose<Int",
        ),
        (
            "bad-owner-argument",
            "is not a subtype of",
            "\"wrong owner argument\"",
        ),
    ] {
        let source = fixture(case);
        let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
            .expect_err(case);
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        let span = error
            .span
            .expect("call diagnostics retain their consumer span");
        let start = source.rfind(token).unwrap() as u32;
        assert_eq!(error.file, 0, "{case}");
        assert!(span.start <= start && span.end > start, "{case}: {span:?}");
    }
}
