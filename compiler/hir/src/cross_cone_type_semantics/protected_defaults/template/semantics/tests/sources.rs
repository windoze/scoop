use super::*;

#[test]
fn nominal_member_this_uses_only_the_direct_owner_and_correct_binder_frame() {
    for (generic_owner, generic_callable, static_nested) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (true, false, true),
        (true, true, true),
    ] {
        let case = Case::method(generic_owner, generic_callable, static_nested);
        let template = case.template();
        let mut authority = Authority::new(&case, &template);
        case.validate(&template, &mut authority).unwrap();
        let mut missing = template;
        missing.receiver = OptionalTemplateReceiverV1::Absent;
        assert!(matches!(
            case.validate(&missing, &mut authority),
            Err(ProtectedDefaultTemplateContractSemanticError::Receiver(
                TemplateReceiverSemanticValidationError::Missing { .. }
            ))
        ));
    }
    let case = Case::method(true, true, true);
    let mut template = case.template();
    let mut authority = Authority::new(&case, &template);
    template.type_parameters = CanonicalBinderUseListV1::try_new(vec![binder(1, 0)]).unwrap();
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::MappingArgument { .. })
    ));
}

#[test]
fn receiver_mapping_rejects_callable_frame_in_place_of_nominal_frame() {
    let case = Case::method(true, true, false);
    let mut template = case.template();
    let mut authority = Authority::new(&case, &template);
    let SourceNominalId::GenericTemplate(origin) = case.record.payload().owner() else {
        panic!("expected generic owner")
    };
    let wrong = SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(binder(0, 0), []),
    };
    template.receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, wrong.clone()).unwrap(),
    );
    template.locals = CanonicalTemplateLocalTableV1::try_new(
        template
            .locals()
            .records()
            .iter()
            .map(|record| {
                if record.selector() == &LocalValueSelector::This {
                    local(LocalValueSelector::This, wrong.clone(), &case.origin)
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::Receiver(
            TemplateReceiverSemanticValidationError::CallableType { .. }
        ))
    ));
}

#[test]
fn constructors_and_source_variant_defaults_have_no_receiver() {
    for case in [Case::constructor(), Case::variant()] {
        let mut template = case.template();
        let mut authority = Authority::new(&case, &template);
        case.validate(&template, &mut authority).unwrap();
        let ty = case.record.payload().result().clone();
        let mut locals = template.locals().records().to_vec();
        locals.push(local(LocalValueSelector::This, ty.clone(), &case.origin));
        template.locals = CanonicalTemplateLocalTableV1::try_new(locals).unwrap();
        template.receiver = OptionalTemplateReceiverV1::Present(
            TemplateReceiverV1::try_new(LocalValueSelector::This, ty).unwrap(),
        );
        assert!(matches!(
            case.validate(&template, &mut authority),
            Err(ProtectedDefaultTemplateContractSemanticError::Receiver(
                TemplateReceiverSemanticValidationError::Unexpected { .. }
            ))
        ));
    }
}

#[test]
fn inherited_defaults_require_the_independent_protected_provider_relation() {
    let case = Case::method(false, false, false);
    let mut template = case.template();
    let mut authority = Authority::new(&case, &template);
    let owner = case.record.payload().owner();
    let key = SourceDeclarationKey::function(
        crate::cross_cone_type_semantics::inheritance::tests::support::site(&[owner]),
        CanonicalIdentifier::new("provider").unwrap(),
        0,
        None,
        vec![template.result().clone(); 2],
    );
    let function = PersistentFunctionId::from_source_declaration(&key).unwrap();
    authority
        .fixture
        .declarations
        .insert(CallableTemplateOrigin::Function(function), key);
    template.definition_root = PersistentLexicalRootV1::Function(function);
    authority.root = template.definition_root();
    case.validate(&template, &mut authority).unwrap();
    authority.inherited = false;
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(
            ProtectedDefaultTemplateContractSemanticError::DefinitionRoot(
                DefaultTemplateRootSemanticValidationError::InheritedRelation(
                    "wrong inherited provider"
                )
            )
        )
    ));
}
