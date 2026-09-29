use super::*;

mod identities;
mod validation;

const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-capture-combinations.scoop"
));

#[test]
fn default_local_captures_keep_source_values_and_body_abi() {
    let output = lower_source(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/nested-local-captures.scoop"
    )))
    .unwrap();
    identities::check_parameter_aliases(&output, "localCall", 1);
    identities::check_parameter_aliases(&output, "localCallGeneric", 1);
    scoop_mir_lower::lower(&output.local).unwrap();
}

#[test]
fn default_local_captures_combine_generics_local_defaults_and_repeated_closures() {
    let output = lower_source(COMBINATIONS).unwrap();
    identities::check_parameter_aliases(&output, "genericCapture", 2);
    identities::check_default_local_and_expansion_values(&output);
    identities::check_unused_default_is_not_a_materialization_root(&output);
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    assert_eq!(output.local.lambdas.len(), 6);
    assert_eq!(output.local.callable_references.len(), 2);
    assert_eq!(
        mir.closure_classes.len(),
        5,
        "lambda bodies are reused while expanded references retain distinct creation sites"
    );
    assert_eq!(mir.closure_invoke_functions.len(), 5);
    assert_eq!(
        selected(&hir::dump(&output.export)),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/nested-capture-combinations.hir.snap"
        ))
    );
    assert_eq!(
        selected(&scoop_mir::dump(&mir)),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/nested-capture-combinations.mir.snap"
        ))
    );
}

fn selected(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = [
                "genericCapture",
                "forward",
                "localDefault",
                "capturedCallback",
                "capturedReference",
                "unused",
                "fun main",
                "$local.",
                "$lambda.",
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

#[test]
fn repeated_defaults_reuse_anonymous_bodies_and_separate_reference_creations() {
    let output = lower_source(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/repeated-default-closures.scoop"
    )))
    .unwrap();
    let module = &output.local;
    assert_eq!(module.anonymous_functions.len(), 2);
    let captures = module
        .anonymous_functions
        .iter()
        .map(|(id, _)| {
            module
                .local_value_identities
                .anonymous_function_capture(id, 0)
                .id()
        })
        .collect::<Vec<_>>();
    assert_eq!(captures[0], captures[1]);
    assert_eq!(module.callable_references.len(), 2);
    let receivers = module
        .callable_references
        .iter()
        .map(|(id, _)| {
            module
                .local_value_identities
                .callable_reference_receiver(id)
                .unwrap()
                .id()
        })
        .collect::<Vec<_>>();
    assert_ne!(receivers[0], receivers[1]);
    let mir = scoop_mir_lower::lower(module).unwrap();
    assert_eq!(mir.closure_classes.len(), 3);
    assert_eq!(mir.closure_invoke_functions.len(), 3);
}
