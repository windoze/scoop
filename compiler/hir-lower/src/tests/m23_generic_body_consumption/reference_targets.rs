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
                assert!(
                    export
                        .callable_references
                        .iter()
                        .any(|(_, reference)| matches!(
                            reference.definition_root,
                            hir::CallableReferenceRoot::Source(_)
                        ))
                );
                if case == "combined" {
                    assert!(
                        export
                            .callable_references
                            .iter()
                            .any(|(_, reference)| matches!(
                                reference.definition_root,
                                hir::CallableReferenceRoot::Persistent(_)
                            ))
                    );
                    assert!(export.local_functions.iter().any(|(_, declaration)| {
                        let hir::LexicalFunctionDefinition::Template(template) =
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

#[test]
fn recursive_reference_receivers_keep_the_final_capture_arguments() {
    use hir::concrete::{Callable, CallableReferenceTarget, CallableTarget, ExprKind};
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut observed = 0;
                for (_, reference) in module.callable_references.iter() {
                    let CallableReferenceTarget::BoundMember { receiver, .. } = &reference.target
                    else {
                        continue;
                    };
                    let (callee, argument_count) = match &receiver.kind {
                        ExprKind::LocalFunctionCall {
                            callee: Callable::Function(function),
                            captures,
                            args,
                            ..
                        } => (*function, captures.len() + args.len()),
                        ExprKind::Call {
                            callee: CallableTarget::Local(Callable::Function(function)),
                            args,
                            ..
                        } => (*function, args.len()),
                        _ => continue,
                    };
                    let function = &module.functions[callee];
                    if !function.name.ends_with("descend") {
                        continue;
                    }
                    assert_eq!(
                        argument_count,
                        function.params.len(),
                        "{case}: recursive receiver retains hidden captures"
                    );
                    assert_eq!(function.capture_parameters.len(), 1);
                    observed += 1;
                }
                assert!(
                    observed > 0,
                    "{case}: recursive calls occur in reference receivers"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
