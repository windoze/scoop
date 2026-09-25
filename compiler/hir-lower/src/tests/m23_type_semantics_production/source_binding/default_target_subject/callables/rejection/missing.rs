use super::*;

#[test]
fn default_callable_targets_require_actual_callee_and_accessor_owner_records() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        for (name, position, missing) in [
            ("direct", 0, "function"),
            ("generic", 1, "generic"),
            ("local", 0, "function"),
            ("getter", 1, "accessor"),
            ("getter", 1, "property"),
            ("extension", 1, "extension"),
            ("lambda", 0, "generated"),
            ("anonymous", 0, "generated"),
            ("reference", 0, "generated"),
        ] {
            let target = target(output, name, position);
            let mut canonical = fixture.foundation.as_canonical().clone();
            match missing {
                "function" => canonical.set_functions(vec![]).unwrap(),
                "generic" => canonical.set_generic_functions(vec![]).unwrap(),
                "accessor" => canonical.set_property_accessors(vec![]).unwrap(),
                "property" => canonical.set_properties(vec![]).unwrap(),
                "extension" => canonical.set_extension_properties(vec![]).unwrap(),
                "generated" => canonical.set_generated_callables(vec![]).unwrap(),
                _ => panic!("record table"),
            }
            let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let error = match fixture
                .source
                .bind_to_foundation(&artifact, &fixture.identities)
            {
                Ok(foundation) => foundation
                    .default_callable_access_subject(&target)
                    .unwrap_err(),
                Err(error) => Error::Foundation(error),
            };
            assert!(
                matches!(
                    error,
                    Error::MissingDeclaration(_)
                        | Error::MissingCallable(_)
                        | Error::Foundation(hir::TypeFoundationBindingError::MissingAccessor(_))
                ),
                "{name} {missing}: {error:?}"
            );
        }
    });
}

#[test]
fn local_and_bound_interface_callables_reject_constructor_origins() {
    with_hir_source(COMBINATIONS, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        let export = output.output().export.module();
        let constructor = export
            .class_constructors
            .iter()
            .find_map(|(id, _)| {
                export.constructor_identities[id]
                    .source_record()
                    .map(|r| r.id())
            })
            .unwrap();
        let declaration = CallableTemplateOrigin::Constructor(constructor);
        let Callable::Bound(bound) = target(output, "interfaceBound", 1) else {
            panic!("bound interface");
        };
        let hir::DefaultBoundCallableSourceV1::Interface {
            bound: bound_type, ..
        } = bound.source()
        else {
            panic!("interface source");
        };
        let wrong = Callable::Bound(hir::DefaultBoundCallableRefV1::new(
            bound.receiver_parameter(),
            hir::DefaultBoundCallableSourceV1::Interface {
                bound: bound_type.clone(),
                member: declaration,
            },
            bound.instantiated_signature().clone(),
        ));
        for wrong in [wrong, Callable::LocalFunction { declaration }] {
            assert!(
                matches!(foundation.default_callable_access_subject(&wrong), Err(Error::CallableOrigin(actual)) if actual == declaration)
            );
        }
    });
}
