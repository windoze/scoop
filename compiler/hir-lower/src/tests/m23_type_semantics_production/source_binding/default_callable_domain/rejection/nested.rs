use super::*;
use hir::{DefaultCallableDeclarationV1 as Declaration, DefaultSourceDomainError as Cause};
use scoop_identity::CallableTemplateOrigin;

#[test]
fn callable_domains_reject_valid_artifact_nested_identities_from_another_default() {
    with_inputs(SOURCE, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let original = support::named(inputs, "Host.direct", 0);
            let Target::Callable(callee) = original.references().callables()[0].target() else { panic!("direct call") };
            let mut local = 0;
            let mut generated = 0;
            for template in inputs.templates.records() {
                let nested = template.index_nested_callables( &scoop_wire::WirePath::root()).unwrap();
                for occurrence in nested.occurrences() {
                    let identity = occurrence.descriptor().identity();
                    let declaration = match identity {
                        hir::DefaultNestedCallableIdentityV1::LocalFunction(CallableTemplateOrigin::Function(id)) => Declaration::Function(id),
                        hir::DefaultNestedCallableIdentityV1::LocalFunction(_) => continue,
                        hir::DefaultNestedCallableIdentityV1::Lambda(id)
                        | hir::DefaultNestedCallableIdentityV1::AnonymousFunction(id)
                        | hir::DefaultNestedCallableIdentityV1::CallableReference(id) => Declaration::Generated(id),
                    };
                    let callee = hir::DefaultCallableRefV1::try_new(declaration, callee.owner().clone(), callee.type_arguments().to_vec()).unwrap();
                    let changed = replacement::call(original, &callee, Domain::universal());
                    let table = super::super::super::default_origins::replace(&inputs.templates, changed);
                    let declarations = parameters.bind_default_declarations(&table, &[]).unwrap();
                    let error = domains.bind_nominal_default_callable_domains(&declarations).unwrap_err();
                    let Error::Target { key, index, error } = error else { panic!("{error:?}") };
                    assert_eq!((key, index), (original.key(), 0));
                    match declaration {
                        Declaration::Generated(_) => { assert!(matches!(*error, Cause::NestedAttachment)); generated += 1; }
                        _ => { assert!(matches!(*error, Cause::NestedOccurrence(actual) if actual == identity)); local += 1; }
                    }
                }
            }
            assert!(local >= 2 && generated >= 6);
        });
    });
}

#[test]
fn callable_reference_domain_rejects_false_local_roles_and_generated_underlying_targets() {
    with_inputs(SOURCE, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let original = support::named(inputs, "Host.reference", 0);
            let hir::DefaultExpressionKindV1::CallableReference(reference) =
                original.body().value().kind()
            else {
                panic!("callable reference")
            };
            let hir::DefaultCallableReferenceTargetV1::Named(callee) = reference.target() else {
                panic!("named target")
            };
            let Declaration::Function(function) = callee.declaration() else {
                panic!("source function")
            };
            let local_template = support::named(inputs, "Host.local", 0);
            let nested = local_template
                .index_nested_callables(&scoop_wire::WirePath::root())
                .unwrap();
            let hir::DefaultNestedCallableIdentityV1::LocalFunction(local) =
                nested.occurrences()[0].descriptor().identity()
            else {
                panic!("local source")
            };
            let generated = hir::DefaultCallableRefV1::try_new(
                Declaration::Generated(reference.invoke()),
                callee.owner().clone(),
                callee.type_arguments().to_vec(),
            )
            .unwrap();
            for (index, target) in [
                hir::DefaultCallableReferenceTargetV1::Local {
                    declaration: local,
                    callee: callee.clone(),
                },
                hir::DefaultCallableReferenceTargetV1::Local {
                    declaration: CallableTemplateOrigin::Function(function),
                    callee: callee.clone(),
                },
                hir::DefaultCallableReferenceTargetV1::Named(generated),
            ]
            .into_iter()
            .enumerate()
            {
                let reference = hir::DefaultCallableReferenceV1::try_new(
                    reference.invoke(),
                    reference.definition_path().clone(),
                    target,
                    reference.function_type().clone(),
                    reference.captures().to_vec(),
                    reference.owner_type_parameter_count(),
                )
                .unwrap();
                let changed = replacement::body(
                    original,
                    hir::DefaultExpressionKindV1::CallableReference(reference),
                    original.references().clone(),
                );
                let table =
                    super::super::super::default_origins::replace(&inputs.templates, changed);
                let declarations = parameters.bind_default_declarations(&table, &[]).unwrap();
                let error = domains
                    .bind_nominal_default_callable_domains(&declarations)
                    .unwrap_err();
                let Error::Target { key, error, .. } = error else {
                    panic!("{error:?}")
                };
                assert_eq!(key, original.key());
                assert!(match (index, *error) {
                    (0, Cause::LocalReference(actual)) => actual == local,
                    (1, Cause::Target(error)) =>
                        matches!(*error, hir::DefaultSourceTargetSubjectError::NestedRole(_)),
                    (2, Cause::Target(error)) => matches!(
                        *error,
                        hir::DefaultSourceTargetSubjectError::CallableRole(_)
                    ),
                    _ => false,
                });
            }
        });
    });
}
