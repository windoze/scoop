use super::*;

pub(super) fn build(fixture: &Fixture, case: Case, position: u32) -> ProtectedDefaultTemplateV1 {
    let key = ProtectedDefaultTemplateKeyV1::try_new(fixture.key.owner(), position).unwrap();
    let local = |selector, ty| {
        TemplateLocalRecordV1::try_new(
            selector,
            ty,
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(fixture.origin.clone()),
        )
        .unwrap()
    };
    let mut locals = vec![
        local(LocalValueSelector::This, fixture.receiver.clone()),
        local(parameter(), fixture.unit.clone()),
    ];
    let expression =
        |kind, ty| DefaultExpressionV1::try_new(kind, ty, fixture.origin.clone()).unwrap();
    let statement = |kind| DefaultStatementV1::try_new(kind, fixture.origin.clone()).unwrap();
    let mut statements = Vec::new();
    if case.flow() {
        let selector = declared_local();
        locals.push(local(selector.clone(), fixture.unit.clone()));
        let declare = statement(DefaultStatementKindV1::ValDecl {
            pattern: DefaultPatternV1::binding(selector.clone()),
            init: Box::new(expression(
                DefaultExpressionKindV1::UnitLiteral,
                fixture.unit.clone(),
            )),
        });
        let read = statement(DefaultStatementKindV1::Expr(Box::new(expression(
            DefaultExpressionKindV1::Local(selector),
            fixture.unit.clone(),
        ))));
        statements = if case == Case::BeforeDefinition {
            vec![read, declare]
        } else {
            vec![declare, read]
        };
    }
    if case.nested() {
        let lambda = DefaultLambdaV1::try_new(
            fixture.lambda,
            nested_path(StructuralDefinitionSiteRole::Lambda),
            fixture.function_type(),
            DefaultCallableBodyTypeArgumentsV1::lexical(),
            vec![DefaultCaptureV1::new(
                parameter(),
                fixture.unit.clone(),
                fixture.origin.clone(),
            )],
            0,
        )
        .unwrap();
        statements.push(statement(DefaultStatementKindV1::Expr(Box::new(
            expression(
                DefaultExpressionKindV1::Lambda(lambda),
                fixture.function_type(),
            ),
        ))));
    }
    if matches!(case, Case::OperationType | Case::BodyEnvelope) {
        let ty = if case == Case::BodyEnvelope {
            SignatureTypeKey::Binder { depth: 0, index: 0 }
        } else {
            fixture.receiver.clone()
        };
        statements.push(statement(DefaultStatementKindV1::Expr(Box::new(
            expression(DefaultExpressionKindV1::UnitLiteral, ty),
        ))));
    }
    ProtectedDefaultTemplateV1::try_new(
        key,
        PersistentLexicalRootV1::try_from(key.owner()).unwrap(),
        definition_path(),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(
            statements,
            expression(
                DefaultExpressionKindV1::Local(parameter()),
                fixture.unit.clone(),
            ),
        )
        .unwrap(),
        fixture.unit.clone(),
        CanonicalBooleanV1::from(case == Case::Contract),
        CanonicalBinderUseListV1::try_new(if case.generic() {
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }]
        } else {
            vec![]
        })
        .unwrap(),
        OptionalTemplateReceiverV1::Present(
            TemplateReceiverV1::try_new(LocalValueSelector::This, fixture.receiver.clone())
                .unwrap(),
        ),
        CanonicalTemplateValueParametersV1::try_new(vec![
            TemplateValueParameterV1::try_new(0, parameter()).unwrap(),
        ])
        .unwrap(),
        references(fixture, case),
        fixture.origin.clone(),
    )
    .unwrap()
}
fn witness(
    fixture: &Fixture,
    case: Case,
    target: PersistentAccessDomainV1,
) -> ProtectedDefaultAccessWitnessV1 {
    if case.generic() {
        ProtectedDefaultAccessWitnessV1::generic_source_metadata(fixture.key.owner()).unwrap()
    } else {
        ProtectedDefaultAccessWitnessV1::param_free(
            fixture.key.owner(),
            PersistentLookupDomainV1::new(fixture.direct.as_ref().unwrap().clone()),
            CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
            PersistentLookupDomainV1::new(target),
        )
        .unwrap()
    }
}
fn uses(indices: &[u32], wrong: bool) -> CanonicalProtectedDefaultExpressionUsesV1 {
    CanonicalProtectedDefaultExpressionUsesV1::try_new(
        indices
            .iter()
            .map(|index| {
                ProtectedDefaultExpressionUseV1::new(
                    *index,
                    if wrong {
                        ProtectedDefaultReceiverUseV1::Explicit {
                            receiver_expression_index: *index,
                        }
                    } else {
                        ProtectedDefaultReceiverUseV1::None
                    },
                )
            })
            .collect(),
    )
    .unwrap()
}
fn references(fixture: &Fixture, case: Case) -> ProtectedDefaultReferenceSetV1 {
    let unit_uses: &[u32] = if case.flow() {
        &[0, 1, 2]
    } else if case.nested() {
        &[1]
    } else {
        &[0]
    };
    let public = PersistentAccessDomainV1::universal();
    let mut types = vec![ProtectedDefaultReferenceV1::new(
        fixture.unit.clone(),
        fixture.origin.clone(),
        witness(
            fixture,
            case,
            if case == Case::ReferenceDomain {
                PersistentAccessDomainV1::empty()
            } else {
                public.clone()
            },
        ),
        uses(unit_uses, case == Case::WrongUse),
    )];
    // The receiver local is metadata-only and still requires source/domain replay.
    if case != Case::MissingMetadata {
        types.push(ProtectedDefaultReferenceV1::new(
            fixture.receiver.clone(),
            fixture.origin.clone(),
            witness(fixture, case, public.clone()),
            uses(&[], false),
        ));
    }
    let callables = if case.nested() {
        types.push(ProtectedDefaultReferenceV1::new(
            fixture.function_type(),
            fixture.origin.clone(),
            witness(fixture, case, public),
            uses(&[0], false),
        ));
        vec![ProtectedDefaultReferenceV1::new(
            ExportDefaultCallableTargetV1::Lambda {
                body: fixture.lambda,
            },
            fixture.origin.clone(),
            witness(fixture, case, fixture.direct.as_ref().unwrap().clone()),
            uses(&[0], false),
        )]
    } else {
        vec![]
    };
    ProtectedDefaultReferenceSetV1::try_new(callables, vec![], types, vec![], vec![], vec![])
        .unwrap()
}
