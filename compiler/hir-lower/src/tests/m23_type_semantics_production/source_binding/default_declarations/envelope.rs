use super::*;
use scoop_identity::{
    NonEmptyVec, StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-envelope.scoop"
));

pub(super) fn rebuild(
    t: &Template,
    locals: Vec<hir::TemplateLocalRecordV1>,
    body: hir::ExportDefaultBodyV1,
) -> Template {
    Template::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        hir::CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        body,
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        t.references().clone(),
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap()
}
fn capture_type(t: &Template, ty: SignatureTypeKey) -> Template {
    let hir::DefaultExpressionKindV1::Lambda(lambda) = t.body().value().kind() else {
        panic!("source default is a lambda");
    };
    let captures = lambda
        .captures()
        .iter()
        .map(|capture| {
            hir::DefaultCaptureV1::new(
                capture.source().clone(),
                ty.clone(),
                capture.first_use_origin().clone(),
            )
        })
        .collect();
    let lambda = hir::DefaultLambdaV1::try_new(
        lambda.body(),
        lambda.definition_path().clone(),
        lambda.function_type().clone(),
        lambda.body_type_arguments().clone(),
        captures,
        lambda.owner_type_parameter_count(),
    )
    .unwrap();
    let body = hir::ExportDefaultBodyV1::try_new(
        t.body().statements().to_vec(),
        hir::DefaultExpressionV1::try_new(
            hir::DefaultExpressionKindV1::Lambda(lambda),
            t.result().clone(),
            t.body().value().definition_origin().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    rebuild(t, t.locals().records().to_vec(), body)
}

#[test]
fn default_declarations_check_non_parameter_local_types_and_scope() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "TypeEnvelopeHost.scoped", 1))
            .unwrap();
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let mut locals = original.locals().records().to_vec();
            let index = locals.iter().position(|local| {
                matches!(local.selector(), LocalValueSelector::LocalDeclaration { .. })
            }).unwrap();
            let old = &locals[index];
            locals[index] = hir::TemplateLocalRecordV1::try_new(
                old.selector().clone(),
                SignatureTypeKey::Binder { depth: 0, index: 0 },
                old.mutable(), old.definition().clone(),
            ).unwrap();
            let changed = replace(&table, rebuild(original, locals, original.body().clone()));
            let Error::Record { key: failed, error } = parameters
                .bind_default_declarations(&changed, &[], &mut meter()).unwrap_err()
            else { panic!("source envelope must fail"); };
            assert_eq!(failed, original.key());
            let Error::LocalType { index: actual, error } = *error else {
                panic!("source local type must fail");
            };
            assert_eq!(actual, index);
            assert!(matches!(*error,
                hir::MeteredSignatureTypeSemanticError::Semantic(
                    hir::SignatureTypeSemanticError::BinderScope(
                        hir::SignatureBinderScopeError::DepthOutOfRange {
                            depth: 0, available_depths: 0,
                        }
                    )
                )
            ));
            let mut locals = original.locals().records().to_vec();
            let wrong = LocalValueSelector::LocalDeclaration {
                path: StructuralDefinitionPath::from_first(
                    StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 99),
                    [],
                ),
            };
            locals.push(hir::TemplateLocalRecordV1::try_new(
                wrong.clone(), original.result().clone(), hir::CanonicalBooleanV1::False,
                hir::TemplateLocalDefinitionV1::Source(original.definition_origin().clone()),
            ).unwrap());
            let changed = replace(&table, rebuild(original, locals, original.body().clone()));
            let Error::Record { error, .. } = parameters
                .bind_default_declarations(&changed, &[], &mut meter()).unwrap_err()
            else { panic!("source scope must fail"); };
            assert!(matches!(*error,
                Error::LocalScope(hir::TemplateLocalScopeValidationError::LocalOutsideDefinitionPath {
                    selector, ..
                }) if selector == wrong
            ));
        });
    });
}

#[test]
fn default_declarations_replay_nested_capture_types_from_independent_nominal_shapes() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "TypeEnvelopeHost.capture", 1))
            .unwrap();
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let array = core.array().persistent();
            let hir::DefaultExpressionKindV1::Lambda(lambda) = original.body().value().kind()
            else {
                panic!("source default is a lambda");
            };
            let expected_origin = lambda.captures()[0].first_use_origin();
            let cases = [
                SignatureTypeKey::Binder { depth: 0, index: 0 },
                SignatureTypeKey::NominalApplication {
                    origin: array,
                    arguments: NonEmptyVec::from_first(
                        original.result().clone(),
                        [original.result().clone()],
                    ),
                },
            ];
            for (index, ty) in cases.into_iter().enumerate() {
                let changed = replace(&table, capture_type(original, ty));
                let Error::Record { key: failed, error } = parameters
                    .bind_default_declarations(&changed, &[], &mut meter())
                    .unwrap_err()
                else {
                    panic!("source capture type must fail");
                };
                assert_eq!(failed, original.key());
                let Error::BodyEnvelope(error) = *error else {
                    panic!("source body type must fail");
                };
                let hir::DefaultBodyProviderEnvelopeSemanticValidationError::Type {
                    site,
                    definition_origin,
                    error,
                } = *error
                else {
                    panic!("source capture type must fail");
                };
                assert_eq!(site, hir::DefaultBodyProviderTypeSiteV1::CaptureValue);
                assert_eq!(*definition_origin, *expected_origin);
                assert!(match (index, *error) {
                    (
                        0,
                        hir::SignatureTypeSemanticError::BinderScope(
                            hir::SignatureBinderScopeError::DepthOutOfRange {
                                depth: 0,
                                available_depths: 0,
                            },
                        ),
                    ) => true,
                    (
                        1,
                        hir::SignatureTypeSemanticError::GenericNominalArity {
                            declaration,
                            expected: 1,
                            actual: 2,
                        },
                    ) => declaration == array,
                    _ => false,
                });
            }
        });
    });
}
