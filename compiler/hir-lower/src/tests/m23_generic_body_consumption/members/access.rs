use super::*;

#[test]
fn imported_generic_members_preserve_protected_access() {
    for case in [
        "access-method",
        "access-super",
        "access-setter",
        "access-property-override",
        "access-secondary",
        "access-lexical",
        "access-abi",
        "access-ordinary",
        "access-object",
    ] {
        eprintln!("protected generic member case: {case}");
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
fn imported_generic_members_enforce_protected_receivers_and_setters() {
    for (case, expected, token) in [
        ("access-base-receiver", "protected method", "secret()"),
        ("access-sibling-receiver", "protected method", "secret()"),
        (
            "access-constructor-error",
            "is not accessible",
            "ProtectedBox<Int>",
        ),
        ("access-setter-error", "setter of property", "state ="),
        ("access-base-setter", "setter of property", "state ="),
        (
            "access-private-setter",
            "setter of property",
            "restricted =",
        ),
        ("access-narrow-override", "visibility", "secret"),
        ("access-public-override", "visibility", "read"),
        ("access-narrow-setter", "visibility", "state"),
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
            .expect("access diagnostics retain their consumer span");
        let start = source.rfind(token).unwrap() as u32;
        assert_eq!(error.file, 0, "{case}");
        assert!(span.start <= start && span.end > start, "{case}: {span:?}");
    }
}
