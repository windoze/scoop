use super::*;
use hir::{DefaultExpressionKindV1 as Expr, DefaultSourceNestedIdentityFailureV1 as Failure};
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-identities.scoop"
));

fn value(template: &Template, kind: Expr) -> Template {
    envelope::rebuild(
        template,
        template.locals().records().to_vec(),
        hir::ExportDefaultBodyV1::try_new(
            template.body().statements().to_vec(),
            hir::DefaultExpressionV1::try_new(
                kind,
                template.result().clone(),
                template.body().value().definition_origin().clone(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn default_source_nested_identities_require_actual_generated_roles_and_paths() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "NestedIdentityHost.lambda", 1))
            .unwrap();
        let anonymous = table
            .get(key(output, "NestedIdentityHost.anonymous", 1))
            .unwrap();
        let reference = table
            .get(key(output, "NestedIdentityHost.reference", 0))
            .unwrap();
        let Expr::Lambda(lambda) = original.body().value().kind() else {
            panic!("expected lambda");
        };
        let Expr::AnonymousFunction(anon) = anonymous.body().value().kind() else {
            panic!("expected anonymous function");
        };
        let Expr::CallableReference(callable) = reference.body().value().kind() else {
            panic!("expected callable reference");
        };
        let wrong_path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 99),
            [],
        );
        let changed = [
            (
                original,
                value(
                    original,
                    Expr::Lambda(
                        hir::DefaultLambdaV1::try_new(
                            anon.body(),
                            lambda.definition_path().clone(),
                            lambda.function_type().clone(),
                            lambda.body_type_arguments().clone(),
                            lambda.captures().to_vec(),
                            lambda.owner_type_parameter_count(),
                        )
                        .unwrap(),
                    ),
                ),
                Failure::Kind,
            ),
            (
                original,
                value(
                    original,
                    Expr::Lambda(
                        hir::DefaultLambdaV1::try_new(
                            lambda.body(),
                            wrong_path,
                            lambda.function_type().clone(),
                            lambda.body_type_arguments().clone(),
                            lambda.captures().to_vec(),
                            lambda.owner_type_parameter_count(),
                        )
                        .unwrap(),
                    ),
                ),
                Failure::DefinitionPath,
            ),
            (
                anonymous,
                value(
                    anonymous,
                    Expr::AnonymousFunction(
                        hir::DefaultAnonymousFunctionV1::try_new(
                            lambda.body(),
                            anon.definition_path().clone(),
                            anon.function_type().clone(),
                            anon.body_type_arguments().clone(),
                            anon.captures().to_vec(),
                            anon.owner_type_parameter_count(),
                        )
                        .unwrap(),
                    ),
                ),
                Failure::Kind,
            ),
            (
                reference,
                value(
                    reference,
                    Expr::CallableReference(
                        hir::DefaultCallableReferenceV1::try_new(
                            lambda.body(),
                            callable.definition_path().clone(),
                            callable.target().clone(),
                            callable.function_type().clone(),
                            callable.captures().to_vec(),
                            callable.owner_type_parameter_count(),
                        )
                        .unwrap(),
                    ),
                ),
                Failure::Kind,
            ),
        ];
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            for (original, changed, expected) in changed {
                let changed = replace(&table, changed);
                let Error::Record { key: actual, error } = parameters
                    .bind_default_declarations(&changed, &[])
                    .unwrap_err()
                else {
                    panic!("expected nested identity error");
                };
                assert_eq!(actual, original.key());
                assert!(
                    matches!(*error, Error::NestedIdentity { reason, .. } if reason == expected),
                    "{error:?}"
                );
            }
        });
    });
}

#[test]
fn source_foundation_publishes_unmaterialized_callable_reference_keys_and_origins() {
    with_sources(SOURCE, |output, fixture, _, _| {
        let ordinary = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        assert_eq!(
            fixture
                .foundation
                .as_canonical()
                .counts()
                .generated_callables,
            ordinary.counts().generated_callables
        );
        let ordinary = hir::OdrFreeHirFoundation::try_new(ordinary).unwrap();
        let original = source_template(output, "NestedIdentityHost.reference", 0);
        let Expr::CallableReference(reference) = original.body().value().kind() else {
            panic!("expected source callable reference");
        };
        let subject =
            scoop_identity::DefinitionOriginSubject::GeneratedCallable(reference.invoke());
        let origin = fixture.foundation.definition_origin(subject).unwrap();
        assert_eq!(ordinary.definition_origin(subject), Some(origin));
        ordinary
            .source_record(origin.origin().source())
            .unwrap()
            .require_points([
                origin.origin().span().start_byte(),
                origin.origin().span().end_byte(),
            ])
            .unwrap();
        assert_eq!(
            origin.origin(),
            original.body().value().definition_origin().origin()
        );
        let key = fixture.identities.canonical_key::<scoop_identity::PersistentGeneratedCallableId, scoop_identity::GeneratedCallableKey>(reference.invoke()).unwrap();
        assert!(
            matches!(key.as_ref(), scoop_identity::GeneratedCallableKey::CallableReferenceInvoke { path, .. } if path == reference.definition_path())
        );
    });
}

#[test]
fn default_source_local_function_cannot_claim_an_enclosing_method_identity() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "NestedIdentityHost.lambda", 1))
            .unwrap();
        let Expr::Lambda(lambda) = original.body().value().kind() else {
            panic!("expected lambda");
        };
        let function = hir::DefaultLocalFunctionV1::try_new(
            original.key().owner(),
            lambda.definition_path().clone(),
            lambda.function_type().clone(),
            Vec::new(),
            0,
        )
        .unwrap();
        let statement = hir::DefaultStatementV1::try_new(
            hir::DefaultStatementKindV1::LocalFunction(function),
            original.definition_origin().clone(),
        )
        .unwrap();
        let changed = envelope::rebuild(
            original,
            original.locals().records().to_vec(),
            hir::ExportDefaultBodyV1::try_new(vec![statement], original.body().value().clone())
                .unwrap(),
        );
        let changed = replace(&table, changed);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols).unwrap();
            let Error::Record { error, .. } = parameters.bind_default_declarations(&changed, &[]).unwrap_err() else { panic!("expected local identity error"); };
            assert!(matches!(*error, Error::NestedIdentity { identity: hir::DefaultNestedCallableIdentityV1::LocalFunction(actual), reason: Failure::Kind } if actual == original.key().owner()), "{error:?}");
        });
    });
}
