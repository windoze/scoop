use super::*;

#[test]
fn expanded_default_local_descriptors_keep_callee_signatures_and_caller_values() {
    let source = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-dependency-binders.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-dependency-combinations.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-dependency-binder-calls.scoop"
        )),
    ]
    .join("\n");
    let output = lower_source(&source).unwrap();
    let module = &output.local;
    assert!(!module.local_functions.is_empty());
    for (_, descriptor) in module.local_functions.iter() {
        let function = &module.functions[descriptor.function];
        let signature = &module.function_types[descriptor.function_type];
        assert_eq!(function.return_ty, signature.return_type);
        let parameters = function
            .params
            .iter()
            .skip(function.capture_parameters.len())
            .map(|parameter| parameter.ty)
            .collect::<Vec<_>>();
        assert_eq!(parameters, signature.parameter_types);
    }
    let mut local_references = 0;
    for (id, reference) in module.callable_references.iter() {
        if let hir::concrete::CallableReferenceTarget::Local {
            callee: hir::concrete::Callable::Function(function),
            ..
        } = reference.target
        {
            let parameters = &module.functions[function].capture_parameters;
            assert_eq!(reference.captures.len(), parameters.len());
            for (index, parameter) in parameters.iter().enumerate() {
                assert_eq!(
                    module
                        .local_value_identities
                        .callable_reference_capture(id, index),
                    module
                        .local_value_identities
                        .function_local(function, parameter.local)
                );
            }
            local_references += 1;
        }
    }
    assert!(local_references > 0);
    let mir = scoop_mir_lower::lower(module).unwrap();
    assert_eq!(
        (
            selected(&hir::dump(&output.export)),
            selected(&scoop_mir::dump(&mir))
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/local-dependency-binders.hir.snap"
            ))
            .to_owned(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/local-dependency-binders.mir.snap"
            ))
            .to_owned()
        )
    );
}

fn selected(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = [
                "LocalDependencyHost",
                "DependencyCombinationHost",
                "localDependency",
                "dependencyLambda",
                "dependencyAnonymous",
                "dependencyReference",
                "fun main",
                "$local.",
                "$lambda.",
                "$anonymous.",
                "$reference.",
            ]
            .iter()
            .any(|name| line.contains(name))
                || line.starts_with("  closure ");
        }
        if keep {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
