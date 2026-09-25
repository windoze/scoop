use super::*;

#[test]
fn source_contract_checks_preceding_locals_result_and_suspend_permission() {
    let case = Case::method(false, false, false);
    let template = case.template();
    let mut authority = Authority::new(&case, &template);
    case.validate(&template, &mut authority).unwrap();
    let mut missing = template.clone();
    missing.value_parameters = CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap();
    assert!(matches!(
        case.validate(&missing, &mut authority),
        Err(
            ProtectedDefaultTemplateContractSemanticError::ValueParameters(
                TemplateValueParameterSemanticValidationError::PrefixArity { .. }
            )
        )
    ));
    let mut mutable = template.clone();
    mutable.locals = CanonicalTemplateLocalTableV1::try_new(
        template
            .locals()
            .records()
            .iter()
            .map(|record| {
                TemplateLocalRecordV1::try_new(
                    record.selector().clone(),
                    record.value_type().clone(),
                    if matches!(record.selector(), LocalValueSelector::Parameter { .. }) {
                        CanonicalBooleanV1::True
                    } else {
                        CanonicalBooleanV1::False
                    },
                    record.definition().clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        case.validate(&mutable, &mut authority),
        Err(
            ProtectedDefaultTemplateContractSemanticError::ValueParameters(
                TemplateValueParameterSemanticValidationError::MutableLocal { position: 0 }
            )
        )
    ));
    let mut wrong_type = template.clone();
    wrong_type.locals = CanonicalTemplateLocalTableV1::try_new(
        template
            .locals()
            .records()
            .iter()
            .map(|record| {
                local(
                    record.selector().clone(),
                    case.expected_receiver.clone().unwrap(),
                    &case.origin,
                )
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        case.validate(&wrong_type, &mut authority),
        Err(
            ProtectedDefaultTemplateContractSemanticError::ValueParameters(
                TemplateValueParameterSemanticValidationError::LocalType { position: 0, .. }
            )
        )
    ));
    let mut result = template.clone();
    result.result = case.expected_receiver.clone().unwrap();
    assert!(matches!(
        case.validate(&result, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::ResultMismatch)
    ));
    let mut suspend = template;
    suspend.allows_suspend = CanonicalBooleanV1::True;
    assert!(matches!(
        case.validate(&suspend, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::SuspendPermission)
    ));
}

#[test]
fn source_contract_rejects_wrong_key_provider_and_local_definition_path() {
    let case = Case::method(false, false, false);
    let template = case.template();
    let mut authority = Authority::new(&case, &template);
    let mut wrong = template.clone();
    wrong.key = ProtectedDefaultTemplateKeyV1::try_new(case.key.owner(), 0).unwrap();
    assert!(matches!(
        case.validate(&wrong, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::ParameterTemplate)
    ));
    wrong.key = ProtectedDefaultTemplateKeyV1::try_new(case.key.owner(), 100).unwrap();
    assert!(matches!(
        case.validate(&wrong, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::ParameterOutOfRange { .. })
    ));
    authority.provider = DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap();
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::ProviderOwnerShape)
    ));
    authority.provider = case.provider;
    let mut bad_local = template.clone();
    let mut locals = bad_local.locals().records().to_vec();
    locals.push(local(
        LocalValueSelector::LocalDeclaration {
            path: template.definition_path().clone(),
        },
        template.result().clone(),
        &case.origin,
    ));
    bad_local.locals = CanonicalTemplateLocalTableV1::try_new(locals).unwrap();
    assert!(matches!(
        case.validate(&bad_local, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::LocalScope(_))
    ));
    let mut bad_path = template;
    bad_path.definition_path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::LocalDeclaration, 0),
        [],
    );
    assert!(matches!(
        case.validate(&bad_path, &mut authority),
        Err(
            ProtectedDefaultTemplateContractSemanticError::DefinitionRoot(
                DefaultTemplateRootSemanticValidationError::InvalidDefinitionPathRole { .. }
            )
        )
    ));
}
