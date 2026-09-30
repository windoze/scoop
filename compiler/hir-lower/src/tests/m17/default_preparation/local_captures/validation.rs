use super::*;
use hir::concrete;

fn rebuild(
    output: &hir::Output,
    functions: &la_arena::Arena<concrete::Function>,
) -> Result<concrete::LocalValueIdentities, concrete::LocalValueIdentityError> {
    concrete::LocalValueIdentities::from_callables(inputs(output, functions))
}

fn inputs<'a>(
    output: &'a hir::Output,
    functions: &'a la_arena::Arena<concrete::Function>,
) -> concrete::LocalValueIdentityInputs<'a> {
    let module = &output.local;
    concrete::LocalValueIdentityInputs {
        source_files: &output.export.source_files,
        source_contexts: &output.export.source_context_identities,
        callable_applications: &module.callable_applications,
        functions,
        lambdas: &module.lambdas,
        anonymous_functions: &module.anonymous_functions,
        lexical_local_values: &[],
        callable_references: &module.callable_references,
        class_constructors: &module.class_constructors,
        struct_constructors: &module.struct_constructors,
    }
}

#[test]
fn body_capture_identity_validation_rejects_broken_parameter_and_binding_relations() {
    let output = lower_source(
        "fun main() { val value = 1; fun copy(arg: Int): Int { return value }; copy(2) }",
    )
    .unwrap();
    let (function, _) = output
        .local
        .functions
        .iter()
        .find(|(_, f)| !f.capture_parameters.is_empty())
        .unwrap();
    rebuild(&output, &output.local.functions).unwrap();
    let mut functions = output.local.functions.clone();
    functions[function].capture_parameters[0].local = functions[function].params[1].local;
    assert!(matches!(
        rebuild(&output, &functions),
        Err(concrete::LocalValueIdentityError::MissingCaptureParameter { .. })
    ));
    let mut functions = output.local.functions.clone();
    functions[function].capture_parameters[0].binding = concrete::BindingId::from_raw(u32::MAX);
    assert!(matches!(
        rebuild(&output, &functions),
        Err(concrete::LocalValueIdentityError::MissingCapturedValue { .. })
    ));
    let mut functions = output.local.functions.clone();
    let capture = functions[function].capture_parameters[0];
    functions[function].capture_parameters.push(capture);
    functions[function].params[1].local = capture.local;
    assert!(matches!(
        rebuild(&output, &functions),
        Err(concrete::LocalValueIdentityError::DuplicateCaptureParameter { .. })
    ));
}

#[test]
fn repeated_bound_receivers_reject_conflicting_definition_origins() {
    let output = lower_source("public fun String.copy(): String = this\npublic fun callback(seed: String, value: () -> String = seed::copy): () -> String = value\nfun main() { callback(\"first\"); callback(\"second\") }").unwrap();
    rebuild(&output, &output.local.functions).unwrap();
    let mut references = output.local.callable_references.clone();
    let identity = references.iter().next().unwrap().1.identity.clone();
    let (second, _) = references.iter().nth(1).unwrap();
    references[second].identity = identity;
    references[second].origin.span.start += 1;
    let mut input = inputs(&output, &output.local.functions);
    input.callable_references = &references;
    assert!(matches!(
        concrete::LocalValueIdentities::from_callables(input),
        Err(concrete::LocalValueIdentityError::DuplicateSelector { .. })
    ));
}
