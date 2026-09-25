use super::*;
use crate::cross_cone_interface::expression_test_support::Fixture as ExpressionFixture;

#[test]
fn default_literal_local_and_all_reference_origins_are_independent_inline_uses() {
    let fixture = fixture();
    assert_eq!(fixture.expected.len(), 15);
    assert_eq!(fixture.validate(&fixture.declared()).unwrap(), 15);
    // The result literal has no named reference. Its origin still belongs to field 7.
    let result_origin = origin(ConeIdentity::CORE, 5);
    let declared = CanonicalExportDefinitionSourcesV1::try_new(
        fixture
            .declared()
            .sources()
            .iter()
            .filter(|source| *source != &result_origin)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        fixture.validate(&declared),
        Err(TypeDefinitionSourceClosureError::Missing { .. })
    ));
}
pub(super) fn fixture() -> Fixture {
    let f = ExpressionFixture::new();
    let mut fixture = Fixture::default();
    let owner = CallableTemplateOrigin::Function(f.function);
    let key = ProtectedDefaultTemplateKeyV1::try_new(owner, 1).unwrap();
    let ty = SignatureTypeKey::Nominal(f.type_id);
    let local = f.local();
    let root = origin(ConeIdentity::CORE, 1);
    let source_local = origin(ConeIdentity::CORE, 2);
    let statement_origin = origin(ConeIdentity::CORE, 3);
    let statement_value = origin(ConeIdentity::CORE, 4);
    let result_origin = origin(ConeIdentity::CORE, 5);
    for (site, source) in [
        (DefaultSite::Root, &root),
        (DefaultSite::Local(local.clone()), &source_local),
        (
            DefaultSite::Body(DefaultBodyOriginSiteV1::Statement),
            &statement_origin,
        ),
        (
            DefaultSite::Body(DefaultBodyOriginSiteV1::Expression),
            &statement_value,
        ),
        (
            DefaultSite::Body(DefaultBodyOriginSiteV1::Expression),
            &result_origin,
        ),
    ] {
        fixture.expect(UseKey::Default(key, site), source);
    }
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            local.clone(),
            ty.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(source_local),
        )
        .unwrap(),
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::Synthetic {
                path: default_path(),
                role: SyntheticLocalRole::Temporary,
            },
            ty.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Synthetic,
        )
        .unwrap(),
    ])
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Expr(Box::new(
                    DefaultExpressionV1::try_new(
                        DefaultExpressionKindV1::UnitLiteral,
                        ty.clone(),
                        statement_value,
                    )
                    .unwrap(),
                )),
                statement_origin,
            )
            .unwrap(),
        ],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            ty.clone(),
            result_origin,
        )
        .unwrap(),
    )
    .unwrap();
    let references = references(&mut fixture, &f, key, &ty);
    let template = ProtectedDefaultTemplateV1::try_new(
        key,
        PersistentLexicalRootV1::Function(f.function),
        default_path(),
        locals,
        body,
        ty.clone(),
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(vec![
            TemplateValueParameterV1::try_new(0, local).unwrap(),
        ])
        .unwrap(),
        references,
        root,
    )
    .unwrap();
    fixture.defaults = CanonicalProtectedDefaultTemplatesV1::try_new(vec![template]).unwrap();
    let second = CallableTemplateOrigin::Constructor(f.constructor);
    let calling = [
        (
            owner,
            vec![
                ProtectedParameterCallingV1::Required,
                ProtectedParameterCallingV1::Default { template: key },
                ProtectedParameterCallingV1::VarargEmpty {
                    element_type: ty.clone(),
                },
            ],
        ),
        (
            second,
            vec![ProtectedParameterCallingV1::VarargDefault {
                element_type: ty.clone(),
                template: ProtectedDefaultTemplateKeyV1::try_new(second, 0).unwrap(),
            }],
        ),
    ];
    let mut protocols = Vec::new();
    let mut next_origin = 12;
    for (owner, calling) in calling {
        let parameters = calling
            .into_iter()
            .enumerate()
            .map(|(index, calling)| {
                let source = origin(ConeIdentity::CORE, next_origin);
                next_origin += 1;
                fixture.expect(UseKey::Parameter(owner, index), &source);
                ProtectedSourceParameterV1::new(
                    CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                    ty.clone(),
                    calling,
                    source,
                )
            })
            .collect();
        protocols.push(
            ProtectedCallableSourceInterfaceV1::try_new(
                owner,
                CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
            )
            .unwrap(),
        );
    }
    // This test checks origin transport, not source/default-key closure or executable bodies.
    fixture.sources = CanonicalProtectedCallableSourceInterfacesV1::try_new(protocols).unwrap();
    fixture
}
fn references(
    fixture: &mut Fixture,
    f: &ExpressionFixture,
    key: ProtectedDefaultTemplateKeyV1,
    ty: &SignatureTypeKey,
) -> ProtectedDefaultReferenceSetV1 {
    let witness = ProtectedDefaultAccessWitnessV1::param_free(
        key.owner(),
        PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
        CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
        PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
    )
    .unwrap();
    let uses = CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![]).unwrap();
    macro_rules! reference {
        ($target:expr, $index:literal, $kind:ident) => {{
            let source = origin(ConeIdentity::CORE, $index);
            fixture.expect(UseKey::Default(key, DefaultSite::$kind), &source);
            ProtectedDefaultReferenceV1::new($target, source, witness.clone(), uses.clone())
        }};
    }
    ProtectedDefaultReferenceSetV1::try_new(
        vec![reference!(
            ExportDefaultCallableTargetV1::Callable(f.callable()),
            6,
            Callable
        )],
        vec![reference!(
            DefaultConstructorRefV1::Struct {
                declaration: f.constructor,
                owner_type: ty.clone()
            },
            7,
            Constructor
        )],
        vec![reference!(ty.clone(), 8, Type)],
        vec![reference!(f.property, 9, Global)],
        vec![reference!(f.object, 10, Singleton)],
        vec![reference!(
            DefaultFieldRefV1::Struct {
                declaration: f.field,
                owner_type: ty.clone()
            },
            11,
            Field
        )],
    )
    .unwrap()
}
