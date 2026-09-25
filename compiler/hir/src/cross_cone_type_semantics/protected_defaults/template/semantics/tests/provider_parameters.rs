use super::*;
type Error = ProtectedDefaultTemplateContractSemanticError<&'static str>;

fn collapsed() -> (Case, ProtectedDefaultTemplateV1, Authority) {
    let (case, mut template, authority) =
        provider_receiver::inherited(2, Some(vec![binder(0, 0), binder(0, 1)]));
    template.result = binder(0, 1);
    replace_prefix(&mut template, binder(0, 0));
    (case, template, authority)
}
fn replace_prefix(template: &mut ProtectedDefaultTemplateV1, ty: SignatureTypeKey) {
    template.locals = CanonicalTemplateLocalTableV1::try_new(
        template
            .locals()
            .records()
            .iter()
            .map(|record| {
                if record.selector()
                    == &(LocalValueSelector::Parameter {
                        declaration_index: 0,
                    })
                {
                    local(
                        record.selector().clone(),
                        ty.clone(),
                        template.definition_origin(),
                    )
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
}

#[test]
fn provider_parameters_preserve_raw_binders_under_collapsed_substitution() {
    let (case, template, mut authority) = collapsed();
    case.validate(&template, &mut authority).unwrap();
    let mut wrong = template.clone();
    wrong.result = binder(0, 0);
    assert!(matches!(
        case.validate(&wrong, &mut authority),
        Err(Error::ProviderResultMismatch)
    ));
    let mut wrong = template;
    replace_prefix(&mut wrong, binder(0, 1));
    assert!(matches!(
        case.validate(&wrong, &mut authority),
        Err(Error::ValueParameters(
            TemplateValueParameterSemanticValidationError::LocalType { position: 0, .. }
        ))
    ));
}

#[test]
fn provider_parameters_check_position_arity_and_the_complete_tail() {
    let (case, template, mut authority) = collapsed();
    authority.provider_position = 0;
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(Error::ProviderParameterPosition)
    ));
    authority.provider_position = 2;
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(Error::Foundation("wrong provider position"))
    ));
    authority.provider_position = 1;
    let mut parameters = authority.provider_parameters.parameters().to_vec();
    parameters.push(SourceParameterShapeV1::new(
        CanonicalIdentifier::new("tail").unwrap(),
        binder(0, 0),
    ));
    authority.provider_parameters = CanonicalSourceParameterShapesV1::try_new(parameters).unwrap();
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(Error::ProviderParameterArity)
    ));
    let mut published = case.source.parameters().parameters().to_vec();
    published.push(ProtectedSourceParameterV1::new(
        CanonicalIdentifier::new("tail").unwrap(),
        case.expected_receiver.clone().unwrap(),
        ProtectedParameterCallingV1::Required,
        case.origin.clone(),
    ));
    // Exercise the complete parameter comparison directly: no prefix/result
    // check can observe a parameter occurring after the default position.
    assert!(matches!(
        super::super::parameters::validate(
            &template,
            &published,
            authority.provider,
            &mut authority
        ),
        Err(Error::ProviderParameterType { index: 2 })
    ));
}
