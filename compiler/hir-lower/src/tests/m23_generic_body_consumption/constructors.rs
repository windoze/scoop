use super::*;

fn fixture(name: &str) -> String {
    let mut source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-constructor-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap();
    if name == "provider" {
        source.push_str(
            fixture("provider-base")
                .strip_prefix("package templates.constructors\n")
                .unwrap(),
        );
    }
    source
}

#[test]
fn imported_generic_constructors_keep_initialization_and_provider_identity() {
    for case in [
        "standalone",
        "structure",
        "struct-secondary",
        "class-secondary",
        "class-terminal",
        "default-base",
        "captures",
        "inferred-defaults",
        "local-base",
        "large-value",
        "combined",
        "nested",
    ] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                assert!(export.structs.iter().all(|(_, value)| value.name != "Pair"));
                assert!(
                    export
                        .classes
                        .iter()
                        .all(|(_, value)| { !["Box", "Derived"].contains(&value.name.as_str()) })
                );
                let local = output.output().local.module();
                assert!(
                    !local.class_constructors.is_empty() || !local.struct_constructors.is_empty()
                );
                for (_, constructor) in local.class_constructors.iter() {
                    if let scoop_identity::CallableMaterializationContext::Application(id) =
                        constructor.materialization.context()
                    {
                        let application = local.callable_applications.get(id).unwrap();
                        assert!(matches!(
                            application.key().origin(),
                            scoop_identity::CallableTemplateOrigin::Constructor(_)
                        ));
                    }
                }
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            },
        )
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}

#[test]
fn imported_generic_constructors_enforce_source_call_rules() {
    for (case, expected, token) in [
        ("bad-kind", "must satisfy `value`", "ValueOnly"),
        (
            "bad-arity",
            "expects 1 type argument(s), found 2",
            "Pair<Int",
        ),
        ("private-constructor", "is not accessible", "Locked<Int"),
        (
            "immutable-property",
            "cannot assign to immutable property `echoed`",
            "echoed",
        ),
    ] {
        let source = fixture(case);
        let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
            .expect_err(case);
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        assert_eq!(error.file, 0, "{case}");
        let span = error.span.expect("source call diagnostics have a location");
        let start = source.rfind(token).unwrap() as u32;
        assert!(span.start <= start && span.end > start, "{case}: {span:?}");
    }
}
