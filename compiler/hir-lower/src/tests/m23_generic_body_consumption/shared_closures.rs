use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-closure-bodies")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn closure_occurrences_keep_original_bodies_and_complete_arguments() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().export.module();
                let closures = module
                    .lambdas
                    .values()
                    .map(|lambda| {
                        (
                            lambda.definition,
                            &lambda.body_type_arguments,
                            lambda.owner_type_param_count,
                        )
                    })
                    .chain(module.anonymous_functions.values().map(|anonymous| {
                        (
                            anonymous.definition,
                            &anonymous.body_type_arguments,
                            anonymous.owner_type_param_count,
                        )
                    }));
                let mut source_count = 0;
                let mut imported = std::collections::HashMap::new();
                for (definition, arguments, owner_count) in closures {
                    let parameter_count = match definition {
                        hir::LexicalFunctionDefinition::Source { function, .. } => {
                            source_count += 1;
                            module.functions[function].type_param_count()
                        }
                        hir::LexicalFunctionDefinition::Template(template) => {
                            *imported.entry(template).or_insert(0) += 1;
                            let definition = &module.imported_generic_templates[template];
                            assert!(matches!(
                                definition.declaration,
                                hir::ImportedCallableTemplateOrigin::Closure { .. }
                            ));
                            assert!(matches!(
                                arguments,
                                hir::CallableBodyTypeArguments::Explicit(_)
                            ));
                            definition.type_parameters.len()
                        }
                    };
                    assert_eq!(owner_count, parameter_count);
                    if let hir::CallableBodyTypeArguments::Explicit(arguments) = arguments {
                        assert_eq!(arguments.len(), parameter_count);
                    }
                }
                assert!(source_count > 0);
                if case == "combined" {
                    assert!(
                        imported.values().any(|count| *count > 1),
                        "default occurrences reuse original bodies"
                    );
                    assert!(module.lambdas.values().any(|lambda| matches!(
                        lambda.definition,
                        hir::LexicalFunctionDefinition::Template(_)
                    )));
                    assert!(
                        module
                            .anonymous_functions
                            .values()
                            .any(|anonymous| matches!(
                                anonymous.definition,
                                hir::LexicalFunctionDefinition::Template(_)
                            ))
                    );
                } else {
                    assert!(
                        imported.is_empty(),
                        "standalone closures use current declarations"
                    );
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
