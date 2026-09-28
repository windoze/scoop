use super::*;

#[test]
fn imported_generic_extension_properties_preserve_accessor_applications() {
    for case in [
        "extension-read",
        "extension-write",
        "extension-write-only",
        "extension-abi",
        "extension-inherited",
        "extension-captured",
        "extension-overloads",
        "extension-order",
    ] {
        eprintln!("generic extension property case: {case}");
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                let accessors = export
                    .imported_generic_templates
                    .values()
                    .filter_map(|template| match template.declaration {
                        hir::ImportedCallableTemplateOrigin::ExtensionAccessor(id) => {
                            Some((id, template.signature.parameters.len()))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(!accessors.is_empty(), "{case}");
                let local = output.output().local.module();
                let mut bodies = 0;
                for (_, function) in local.functions.iter() {
                    let scoop_identity::CallableTemplateOwner::Accessor(id) =
                        function.materialization.template()
                    else {
                        continue;
                    };
                    let Some((_, parameter_count)) =
                        accessors.iter().find(|(accessor, _)| *accessor == id)
                    else {
                        continue;
                    };
                    assert!(matches!(
                        function.receiver,
                        hir::concrete::FunctionReceiver::Extension(_)
                    ));
                    let scoop_identity::CallableMaterializationContext::Application(application) =
                        function.materialization.context()
                    else {
                        panic!("{case}: a generic extension accessor has its own application")
                    };
                    let key = local.callable_applications.get(application).unwrap().key();
                    assert_eq!(
                        key.origin(),
                        scoop_identity::CallableTemplateOrigin::Accessor(id)
                    );
                    assert_eq!(
                        key.instantiation_owner(),
                        scoop_identity::CallableInstantiationOwner::NoOwner
                    );
                    assert!(key.callable_arguments().has_arguments());
                    if case == "extension-write-only" {
                        assert_eq!(
                            *parameter_count, 2,
                            "writing does not materialize the getter"
                        );
                    }
                    bodies += 1;
                }
                assert!(bodies > 0, "{case} materializes actual accessors");
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn imported_generic_extension_properties_enforce_receiver_and_setter_rules() {
    for (case, expected, token) in [
        (
            "extension-readonly",
            "cannot assign to immutable property",
            "extensionValue =",
        ),
        (
            "extension-private-setter",
            "setter of property",
            "extensionRestricted =",
        ),
        (
            "extension-rhs-type",
            "cannot assign value of type",
            "\"wrong\"",
        ),
        (
            "extension-result-type",
            "body of `result` must be of type String, found Int",
            "Box<Int>(42).extensionValue",
        ),
        (
            "extension-kind-bound",
            "must satisfy `value`",
            "extensionValueOnly",
        ),
    ] {
        let source = fixture(case);
        let errors = with_provider_consumer(&fixture("provider"), &source, |_, _, _, _, _| ())
            .expect_err(case);
        assert_eq!(errors.len(), 1, "{case}: {errors:?}");
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("{case}: {errors:?}"));
        let span = error
            .span
            .expect("property diagnostics retain the consumer span");
        let start = source.rfind(token).unwrap() as u32;
        assert_eq!(error.file, 0, "{case}");
        assert!(span.start <= start && span.end > start, "{case}: {span:?}");
    }
}
