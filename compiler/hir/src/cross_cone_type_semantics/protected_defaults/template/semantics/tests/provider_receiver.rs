use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;

fn set_receiver(template: &mut ProtectedDefaultTemplateV1, receiver: SignatureTypeKey) {
    template.receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, receiver.clone()).unwrap(),
    );
    template.locals = CanonicalTemplateLocalTableV1::try_new(
        template
            .locals()
            .records()
            .iter()
            .map(|record| {
                if record.selector() == &LocalValueSelector::This {
                    TemplateLocalRecordV1::try_new(
                        record.selector().clone(),
                        receiver.clone(),
                        record.mutable(),
                        record.definition().clone(),
                    )
                    .unwrap()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
}

pub(super) fn inherited(
    generic_arity: u32,
    parameter_types: Option<Vec<SignatureTypeKey>>,
) -> (Case, ProtectedDefaultTemplateV1, Authority) {
    let mut case = Case::method(false, false, false);
    let key = SourceDeclarationKey::nominal(
        site(&[]),
        CanonicalIdentifier::new("ProviderBase").unwrap(),
        SourceNominalKind::Class,
        generic_arity,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    case.fixture.graph.keys.insert(owner, key);
    case.fixture.graph.access.insert(
        owner,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Public,
            vec![],
            case.origin.clone(),
        )
        .unwrap(),
    );
    case.fixture
        .graph
        .origins
        .insert(owner, case.origin.clone());
    if generic_arity == 0 {
        let base = case.fixture.class("ProviderBase");
        let child = case.fixture.class("Owner");
        case.fixture.graph.edges(child, Some(base), &[]);
    }
    let mut template = case.template();
    let parameter_types = parameter_types.unwrap_or_else(|| vec![template.result().clone(); 2]);
    let provider_key = SourceDeclarationKey::function(
        site(&[owner]),
        CanonicalIdentifier::new("provider").unwrap(),
        0,
        None,
        parameter_types.clone(),
    );
    let function = PersistentFunctionId::from_source_declaration(&provider_key).unwrap();
    case.fixture
        .declarations
        .insert(CallableTemplateOrigin::Function(function), provider_key);
    template.definition_root = PersistentLexicalRootV1::Function(function);
    let receiver = match owner {
        SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
        SourceNominalId::GenericTemplate(origin) => SignatureTypeKey::NominalApplication {
            origin,
            arguments: NonEmptyVec::from_first(
                binder(0, 0),
                (1..generic_arity).map(|index| binder(0, index)),
            ),
        },
    };
    set_receiver(&mut template, receiver.clone());
    template.type_parameters =
        CanonicalBinderUseListV1::try_new(vec![template.result().clone(); generic_arity as usize])
            .unwrap();
    let mut authority = Authority::new(&case, &template);
    authority.provider = DefaultTemplateProviderShapeV1::try_new(generic_arity, 0).unwrap();
    authority.provider_receiver = Some(receiver);
    authority.provider_parameters = CanonicalSourceParameterShapesV1::try_new(
        parameter_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SourceParameterShapeV1::new(
                    CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                    ty,
                )
            })
            .collect(),
    )
    .unwrap();
    (case, template, authority)
}

#[test]
fn inherited_default_keeps_the_base_receiver_in_its_original_binder_frame() {
    for generic in [false, true] {
        let (case, mut template, mut authority) = inherited(u32::from(generic), None);
        case.validate(&template, &mut authority).unwrap();
        assert_eq!(authority.inherited_calls, 1);
        assert_ne!(authority.provider_receiver, case.expected_receiver);
        set_receiver(&mut template, case.expected_receiver.clone().unwrap());
        assert!(matches!(
            case.validate(&template, &mut authority),
            Err(ProtectedDefaultTemplateContractSemanticError::Receiver(
                TemplateReceiverSemanticValidationError::CallableType { .. }
            ))
        ));
    }
}

#[test]
fn inherited_relation_checks_the_complete_mapping_even_for_unused_binders() {
    let (case, mut template, mut authority) = inherited(1, None);
    case.validate(&template, &mut authority).unwrap();
    // The value locals and result have no binders, so their type checks cannot
    // establish this owner-to-provider substitution.
    template.type_parameters =
        CanonicalBinderUseListV1::try_new(vec![case.expected_receiver.clone().unwrap()]).unwrap();
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
    assert_eq!(authority.inherited_calls, 2);
}

#[test]
fn direct_defaults_require_identity_mapping_and_the_independent_source_receiver() {
    let case = Case::method(true, true, false);
    let mut template = case.template();
    let mut authority = Authority::new(&case, &template);
    template.type_parameters =
        CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(1, 0)]).unwrap();
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::DirectMapping { index: 0 })
    ));
    template = case.template();
    authority.provider_receiver = None;
    assert!(matches!(
        case.validate(&template, &mut authority),
        Err(ProtectedDefaultTemplateContractSemanticError::ProviderOwnerReceiver)
    ));
    assert_eq!(authority.inherited_calls, 0);
}
