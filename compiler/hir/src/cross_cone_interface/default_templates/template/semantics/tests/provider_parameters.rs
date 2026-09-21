use super::*;
type Error = ExportDefaultTemplateContractSemanticValidationError<AuthorityError>;

fn publishing(
    fixture: &mut Fixture,
    types: Vec<SignatureTypeKey>,
) -> (
    CallableInterfaceRecordV1,
    CallableSourceInterfaceV1,
    Authority,
) {
    let declaration = SourceDeclarationKey::function(
        top_level_site(),
        identifier("collapsedDefault"),
        2,
        None,
        types.clone(),
    );
    fixture.key = ExportDefaultTemplateKeyV1::new(
        CallableTemplateOrigin::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(&declaration).unwrap(),
        ),
        1,
    );
    fixture.identity = CallableDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::TopLevel,
        2,
        0,
        None,
        types.clone(),
    );
    let callable = CallableInterfaceRecordV1::try_new(
        fixture.key.owner(),
        PublicDeclarationOwnerV1::TopLevel,
        binders(2),
        None,
        source_shapes(types.clone()),
        binder(0, 0),
        effects(Effect::Ordinary),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let source = CallableSourceInterfaceV1::try_new(
        fixture.key.owner(),
        CanonicalCallableSourceParametersV1::try_new(
            types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    CallableSourceParameterV1::new(
                        identifier(&format!("p{index}")),
                        ty,
                        if index == 1 {
                            CallableParameterCallingV1::Default {
                                template: fixture.key,
                            }
                        } else {
                            CallableParameterCallingV1::Required
                        },
                        fixture.origin.clone(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
    .unwrap();
    (callable, source, fixture.authority())
}
fn collapsed(fixture: &Fixture) -> ExportDefaultTemplateV1 {
    fixture.template(
        binder(1, 0),
        CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(0, 0)]).unwrap(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    )
}

#[test]
fn provider_parameters_preserve_raw_binders_under_collapsed_substitution() {
    let mut fixture = Fixture::new();
    let (callable, source, mut authority) = publishing(&mut fixture, vec![binder(0, 0); 2]);
    let template = collapsed(&fixture);
    template
        .validate_contract_semantics(&callable, &source, &mut authority)
        .unwrap();
    let mut wrong = template.clone();
    wrong.result = binder(0, 0);
    assert_eq!(
        wrong.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ProviderResultMismatch)
    );
    let wrong = fixture.with_parameter_local(template, binder(1, 0), false);
    assert_eq!(
        wrong.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ValueParameters(
            TemplateValueParameterSemanticValidationError::LocalType {
                position: 0,
                expected: Box::new(binder(0, 0)),
                actual: Box::new(binder(1, 0))
            }
        ))
    );
}

#[test]
fn provider_parameters_check_position_arity_and_the_complete_tail() {
    let mut fixture = Fixture::new();
    let (callable, source, mut authority) =
        publishing(&mut fixture, vec![binder(0, 0), binder(0, 0), binder(0, 1)]);
    let template = collapsed(&fixture);
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ProviderParameterArity)
    );
    authority.provider_parameters = source_shapes(vec![binder(0, 0), binder(1, 0), binder(0, 0)]);
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ProviderParameterType { index: 2 })
    );
    authority.provider_position = 0;
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ProviderParameterPosition)
    );
    authority.provider_position = 3;
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(Error::ProviderParameter(AuthorityError::Provider))
    );
}

#[test]
fn provider_parameter_contract_borrows_a_present_position() {
    let parameters = source_shapes(vec![binder(0, 0), binder(1, 0), binder(0, 1)]);
    let contract = crate::DefaultTemplateProviderParameterV1::try_new(&parameters, 1).unwrap();
    assert!(std::ptr::eq(contract.parameters(), &parameters));
    assert!(std::ptr::eq(
        contract.current(),
        &parameters.parameters()[1]
    ));
    assert_eq!(contract.prefix(), &parameters.parameters()[..1]);
    assert_eq!(contract.position(), 1);
    for position in [3, u32::MAX] {
        assert_eq!(
            crate::DefaultTemplateProviderParameterV1::try_new(&parameters, position).unwrap_err(),
            crate::DefaultTemplateProviderParameterBuildError { position, arity: 3 }
        );
    }
    assert_eq!(
        crate::DefaultTemplateProviderParameterV1::try_new(&source_shapes(vec![]), 0).unwrap_err(),
        crate::DefaultTemplateProviderParameterBuildError {
            position: 0,
            arity: 0
        }
    );
}
