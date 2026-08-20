use super::*;

#[test]
fn generic_default_reference_uses_its_creation_scope() {
    let output = lower_source(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/generic-reference-owner.scoop"
    )))
    .unwrap();
    assert_eq!(output.local.callable_references.len(), 4);
    let contexts = output
        .local
        .functions
        .iter()
        .filter(|(_, f)| matches!(f.name.as_str(), "genericReference" | "unusedReference"))
        .map(|(_, f)| f.materialization.context())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(contexts.len(), 4);
    let main = output
        .local
        .functions
        .iter()
        .find(|(_, f)| f.name == "main")
        .unwrap()
        .1;
    let mut invokes = std::collections::HashSet::new();
    for (_, reference) in output.local.callable_references.iter() {
        assert_eq!(
            reference.identity.materialization().context(),
            main.materialization.context()
        );
        let scoop_identity::GeneratedCallableKey::CallableReferenceInvoke { parent, .. } =
            reference.identity.callable_record().key()
        else {
            panic!("a reference has an invoke key");
        };
        assert_eq!(parent.template(), main.materialization.template());
        assert!(invokes.insert(reference.identity.callable_record().id()));
    }
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    assert_eq!(mir.closure_classes.len(), 4);
}

#[test]
fn generic_default_closures_compose_reordered_binders_through_multiple_expansions() {
    let output = lower_source(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/generic-closure-expansion.scoop"
    )))
    .unwrap();
    let module = &output.local;
    for (function, function_type) in module
        .lambdas
        .iter()
        .map(|(_, f)| (f.function, f.function_type))
        .chain(
            module
                .anonymous_functions
                .iter()
                .map(|(_, f)| (f.function, f.function_type)),
        )
    {
        assert_eq!(
            module.functions[function].return_ty,
            module.function_types[function_type].return_type
        );
    }
    assert!(!module.anonymous_functions.is_empty());
    assert!(!module.lambdas.is_empty());
    for predicate in [
        (|target: &hir::concrete::CallableReferenceTarget| {
            matches!(target, hir::concrete::CallableReferenceTarget::Named(_))
        }) as fn(&hir::concrete::CallableReferenceTarget) -> bool,
        |target| matches!(target, hir::concrete::CallableReferenceTarget::Local { .. }),
        |target| {
            matches!(
                target,
                hir::concrete::CallableReferenceTarget::BoundMember { .. }
            )
        },
        |target| {
            matches!(
                target,
                hir::concrete::CallableReferenceTarget::BoundExtension { .. }
            )
        },
    ] {
        assert!(
            module
                .callable_references
                .iter()
                .any(|(_, r)| predicate(&r.target))
        );
    }
    scoop_mir_lower::lower(module).unwrap();
}
