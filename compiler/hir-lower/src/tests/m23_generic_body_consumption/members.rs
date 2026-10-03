use super::*;

mod access;
mod extensions;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-member-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_members_preserve_applications_and_dispatch() {
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
        "interface-class",
        "interface-abstract",
        "interface-struct",
        "interface-enum",
        "interface-local",
        "interface-abi",
        "interface-properties",
        "interface-value-property",
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
