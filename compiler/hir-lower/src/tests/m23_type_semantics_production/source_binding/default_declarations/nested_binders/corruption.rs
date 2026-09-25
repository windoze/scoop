use super::*;
use hir::{DefaultCallableBodyTypeArgumentsV1 as Arguments, DefaultExpressionKindV1 as Expr};

#[test]
fn nested_owner_binders_reject_forged_counts_for_all_four_descriptor_kinds() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            for name in [
                "BinderHost.lambda",
                "BinderHost.anonymous",
                "BinderHost.reference",
                "BinderHost.local",
                "BinderHost.Plain.callback",
                "BinderHost.Generic.callback",
            ] {
                let original = table.get(key(output, name, 0)).unwrap();
                let index = original.index_nested_callables(&WirePath::root()).unwrap();
                let descriptor = index.occurrences()[0].descriptor();
                let expected = descriptor.owner_type_parameter_count();
                for actual in [0, expected + 1] {
                    let changed = round_trip(corrupt(original, actual, None), output);
                    let changed = replace(&table, changed);
                    let error = parameters
                        .bind_default_declarations(&changed, &[])
                        .unwrap_err();
                    assert_failure(
                        error,
                        original.key(),
                        Failure::OwnerBinderArity { expected, actual },
                    );
                }
            }
        });
    });
}

#[test]
fn expanded_nested_binders_reject_missing_and_extra_explicit_body_arguments() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            for name in [
                "BinderCombinationHost.expanded",
                "BinderCombinationHost.anonymous",
            ] {
                let original = table.get(key(output, name, 0)).unwrap();
                let index = original.index_nested_callables(&WirePath::root()).unwrap();
                let descriptor = index.occurrences()[0].descriptor();
                let hir::DefaultNestedCallableBodyArgumentsV1::Explicit(args) =
                    descriptor.body_arguments()
                else {
                    panic!("expanded body arguments")
                };
                for actual in [2, 4] {
                    let mut arguments = args.to_vec();
                    if actual == 2 {
                        arguments.pop();
                    } else {
                        arguments.push(arguments[0].clone());
                    }
                    let changed = round_trip(
                        corrupt(
                            original,
                            3,
                            Some(Arguments::try_explicit(arguments).unwrap()),
                        ),
                        output,
                    );
                    let changed = replace(&table, changed);
                    let error = parameters
                        .bind_default_declarations(&changed, &[])
                        .unwrap_err();
                    assert_failure(
                        error,
                        original.key(),
                        Failure::BodyBinderArity {
                            expected: 3,
                            actual,
                        },
                    );
                }
            }
        });
    });
}

fn corrupt(t: &Template, count: u32, arguments: Option<Arguments>) -> Template {
    let index = t.index_nested_callables(&WirePath::root()).unwrap();
    let occurrence = &index.occurrences()[0];
    let mut statements = t.body().statements().to_vec();
    let kind = match occurrence.descriptor() {
        Descriptor::LocalFunction(f) => {
            let changed = hir::DefaultLocalFunctionV1::try_new(
                f.declaration(),
                f.definition_path().clone(),
                f.function_type().clone(),
                f.captures().to_vec(),
                count,
            )
            .unwrap();
            statements.insert(
                0,
                hir::DefaultStatementV1::try_new(
                    hir::DefaultStatementKindV1::LocalFunction(changed),
                    occurrence.definition_origin().clone(),
                )
                .unwrap(),
            );
            t.body().value().kind().clone()
        }
        Descriptor::Lambda(f) => Expr::Lambda(
            hir::DefaultLambdaV1::try_new(
                f.body(),
                f.definition_path().clone(),
                f.function_type().clone(),
                arguments.unwrap_or_else(|| f.body_type_arguments().clone()),
                f.captures().to_vec(),
                count,
            )
            .unwrap(),
        ),
        Descriptor::AnonymousFunction(f) => Expr::AnonymousFunction(
            hir::DefaultAnonymousFunctionV1::try_new(
                f.body(),
                f.definition_path().clone(),
                f.function_type().clone(),
                arguments.unwrap_or_else(|| f.body_type_arguments().clone()),
                f.captures().to_vec(),
                count,
            )
            .unwrap(),
        ),
        Descriptor::CallableReference(f) => Expr::CallableReference(
            hir::DefaultCallableReferenceV1::try_new(
                f.invoke(),
                f.definition_path().clone(),
                f.target().clone(),
                f.function_type().clone(),
                f.captures().to_vec(),
                count,
            )
            .unwrap(),
        ),
    };
    let body = hir::ExportDefaultBodyV1::try_new(
        statements,
        hir::DefaultExpressionV1::try_new(
            kind,
            t.result().clone(),
            occurrence.definition_origin().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    envelope::rebuild(t, t.locals().records().to_vec(), body)
}
fn round_trip(template: Template, output: &hir::DependencyHirOutput) -> Template {
    let bytes = encode(&template.index_locals().unwrap()).unwrap();
    let decoded: hir::DecodedDefaultSourceTemplateV1 = decode_canonical(&bytes).unwrap();
    decoded.resolve(&mut identity_closure(output)).unwrap()
}

fn assert_failure(
    error: Error,
    expected_key: hir::ProtectedDefaultTemplateKeyV1,
    expected: Failure,
) {
    let Error::Record { key, error } = error else {
        panic!("expected record error")
    };
    assert_eq!(key, expected_key);
    assert!(
        matches!(*error, Error::NestedIdentity { reason, .. } if reason == expected),
        "{error:?}"
    );
}
