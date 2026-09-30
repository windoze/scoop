use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-reference-targets")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn local_reference_instances_reuse_definitions_and_preserve_capture_signatures() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                let module = output.output().local.module();
                if case == "combined" {
                    assert!(export.local_functions.iter().any(|(_, declaration)| {
                        let hir::LocalFunctionDefinition::Template(template) =
                            declaration.definition
                        else {
                            return false;
                        };
                        export.imported_generic_templates[template].signature.name == "neverCalled"
                    }));
                    assert!(
                        module
                            .functions
                            .iter()
                            .all(|(_, function)| function.name != "neverCalled")
                    );
                }
                let mut functions = std::collections::HashSet::new();
                for (_, descriptor) in module.local_functions.iter() {
                    assert!(
                        functions.insert(descriptor.function),
                        "{case}: one descriptor per implementation"
                    );
                    let function = &module.functions[descriptor.function];
                    assert_eq!(descriptor.span, function.span);
                    let signature = &module.function_types[descriptor.function_type];
                    assert_eq!(function.return_ty, signature.return_type);
                    assert_eq!(
                        function
                            .params
                            .iter()
                            .skip(function.capture_parameters.len())
                            .map(|parameter| parameter.ty)
                            .collect::<Vec<_>>(),
                        signature.parameter_types,
                    );
                }
                let mut references = std::collections::HashMap::new();
                for (id, reference) in module.callable_references.iter() {
                    let hir::concrete::CallableReferenceTarget::Local {
                        local_function,
                        callee: hir::concrete::Callable::Function(function),
                    } = reference.target
                    else {
                        continue;
                    };
                    assert_eq!(module.local_functions[local_function].function, function);
                    *references.entry(function).or_insert(0) += 1;
                    let parameters = &module.functions[function].capture_parameters;
                    assert_eq!(reference.captures.len(), parameters.len());
                    for (index, parameter) in parameters.iter().enumerate() {
                        assert_eq!(
                            module
                                .local_value_identities
                                .callable_reference_capture(id, index),
                            module
                                .local_value_identities
                                .function_local(function, parameter.local),
                        );
                    }
                }
                assert!(
                    references.values().any(|count| *count > 1),
                    "{case}: repeated references share an implementation"
                );
                assert!(
                    references.len() > 1,
                    "{case}: complete arguments keep distinct implementations"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
