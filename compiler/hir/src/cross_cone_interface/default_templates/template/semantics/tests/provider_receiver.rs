use super::*;
mod direct_nominal;
use crate::{PublicNominalKindV1, TemplateReceiverV1};
use scoop_identity::NonEmptyVec;

fn set_receiver(template: &mut ExportDefaultTemplateV1, receiver: SignatureTypeKey) {
    let mut locals = template.locals().records().to_vec();
    locals.retain(|local| local.selector() != &LocalValueSelector::This);
    locals.push(
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::This,
            receiver.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(template.definition_origin().clone()),
        )
        .unwrap(),
    );
    template.locals = CanonicalTemplateLocalTableV1::try_new(locals).unwrap();
    template.receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, receiver).unwrap(),
    );
}

#[test]
fn inherited_defaults_keep_the_raw_provider_receiver() {
    for generic in [false, true] {
        let mut fixture = Fixture::new();
        let mut authority = fixture.authority();
        let mut nominal = |name, arity| {
            let key = SourceDeclarationKey::nominal(
                top_level_site(),
                identifier(name),
                SourceNominalKind::Class,
                arity,
            );
            let source = crate::SourceNominalId::from_source_declaration(&key).unwrap();
            authority.nominals.push((
                source,
                PublicNominalShapeV1::new(PublicNominalKindV1::Class, arity),
            ));
            match source {
                crate::SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
                crate::SourceNominalId::GenericTemplate(origin) => {
                    SignatureTypeKey::NominalApplication {
                        origin,
                        arguments: NonEmptyVec::from_first(binder(1, 0), []),
                    }
                }
            }
        };
        let provider = nominal("Base", u32::from(generic));
        let publishing = nominal("Derived", 0);
        let SignatureTypeKey::Nominal(child_id) = publishing else {
            panic!("publishing class is concrete");
        };
        let owner = PublicDeclarationOwnerV1::Nominal(crate::SourceNominalId::Concrete(child_id));
        let key = SourceDeclarationKey::function(
            owned_site(DefinitionOwnerAtom::Type(child_id)),
            identifier("pick"),
            2,
            None,
            vec![binder(0, 1), binder(0, 0)],
        );
        fixture.key = ExportDefaultTemplateKeyV1::new(
            CallableTemplateOrigin::GenericFunction(
                PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
            ),
            1,
        );
        authority.declaration = fixture.key.owner();
        let provider_atom = match &provider {
            SignatureTypeKey::Nominal(id) => DefinitionOwnerAtom::Type(*id),
            SignatureTypeKey::NominalApplication { origin, .. } => {
                DefinitionOwnerAtom::GenericType(*origin)
            }
            _ => panic!("provider is a source nominal"),
        };
        let provider_parameters = if generic {
            vec![binder(0, 0), binder(1, 0)]
        } else {
            vec![binder(0, 1), binder(0, 0)]
        };
        let key = SourceDeclarationKey::function(
            owned_site(provider_atom),
            identifier("pick"),
            2 - u32::from(generic),
            None,
            provider_parameters,
        );
        fixture.provider = PersistentLexicalRootV1::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
        );
        authority.provider = fixture.provider;
        authority.provider_shape =
            DefaultTemplateProviderShapeV1::try_new(u32::from(generic), 2 - u32::from(generic))
                .unwrap();
        authority.provider_receiver = Some(provider.clone());
        authority.identity = CallableDeclarationIdentityShapeV1::new(
            owner,
            2,
            0,
            None,
            vec![binder(0, 1), binder(0, 0)],
        );
        let callable = CallableInterfaceRecordV1::try_new(
            fixture.key.owner(),
            owner,
            binders(2),
            None,
            source_shapes(vec![binder(0, 1), binder(0, 0)]),
            binder(0, 0),
            effects(Effect::Ordinary),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap();
        let mut template = fixture.template(
            if generic {
                fixture.provider_result()
            } else {
                binder(0, 0)
            },
            fixture.identity_mapping(),
            CanonicalBooleanV1::False,
            CanonicalBooleanV1::False,
        );
        if !generic {
            template = fixture.with_parameter_local(template, binder(0, 1), false);
        }
        set_receiver(&mut template, provider);
        assert_eq!(
            template.validate_contract_semantics(&callable, &fixture.source(), &mut authority),
            Ok(())
        );
        assert_eq!(authority.inherited_validations, 1);
        set_receiver(&mut template, publishing.clone());
        assert!(matches!(
            template.validate_contract_semantics(&callable, &fixture.source(), &mut authority),
            Err(
                ExportDefaultTemplateContractSemanticValidationError::Receiver(
                    TemplateReceiverSemanticValidationError::CallableType { .. }
                )
            )
        ));
        let mut direct = fixture.template(
            binder(0, 0),
            fixture.identity_mapping(),
            CanonicalBooleanV1::False,
            CanonicalBooleanV1::False,
        );
        direct = fixture.with_parameter_local(direct, binder(0, 1), false);
        direct.definition_root = match direct.key().owner() {
            CallableTemplateOrigin::GenericFunction(id) => {
                PersistentLexicalRootV1::GenericFunction(id)
            }
            _ => panic!("fixture has a generic method"),
        };
        set_receiver(&mut direct, publishing.clone());
        authority.provider = direct.definition_root();
        authority.provider_receiver = Some(publishing);
        authority.provider_shape = DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap();
        assert_eq!(
            direct.validate_contract_semantics(&callable, &fixture.source(), &mut authority),
            Ok(())
        );
    }
}

#[test]
fn direct_defaults_require_owner_shape_receiver_and_identity_mapping() {
    let fixture = Fixture::new();
    let callable = fixture.callable(Effect::Ordinary);
    let source = fixture.source();
    let mut template = fixture.template(
        binder(0, 0),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    template = fixture.with_parameter_local(template, binder(0, 1), false);
    template.definition_root = match fixture.key.owner() {
        CallableTemplateOrigin::GenericFunction(id) => PersistentLexicalRootV1::GenericFunction(id),
        _ => panic!("fixture has a generic callable"),
    };
    let mut authority = fixture.authority();
    authority.provider = template.definition_root();
    authority.provider_shape = DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap();
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Ok(())
    );
    assert_eq!(authority.inherited_validations, 0);
    authority.provider_shape = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    assert!(matches!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(ExportDefaultTemplateContractSemanticValidationError::ProviderOwnerBinders)
    ));
    authority.provider_shape = DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap();
    authority.provider_receiver = Some(binder(0, 0));
    assert!(matches!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(ExportDefaultTemplateContractSemanticValidationError::ProviderOwnerReceiver)
    ));
    authority.provider_receiver = None;
    template.type_parameters =
        CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(0, 0)]).unwrap();
    assert!(matches!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(ExportDefaultTemplateContractSemanticValidationError::DirectMapping { index: 1 })
    ));
}

#[test]
fn inherited_relation_receives_even_unused_mapping_arguments() {
    let fixture = Fixture::new();
    let callable = fixture.callable(Effect::Ordinary);
    let mut template = fixture.template(
        binder(0, 0),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    template.key = ExportDefaultTemplateKeyV1::new(fixture.key.owner(), 0);
    template.locals = CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap();
    template.value_parameters = CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap();
    let source = CallableSourceInterfaceV1::try_new(
        fixture.key.owner(),
        CanonicalCallableSourceParametersV1::try_new(vec![
            CallableSourceParameterV1::new(
                identifier("p0"),
                binder(0, 1),
                CallableParameterCallingV1::Default {
                    template: template.key(),
                },
                fixture.origin.clone(),
            ),
            CallableSourceParameterV1::new(
                identifier("p1"),
                binder(0, 0),
                CallableParameterCallingV1::Required,
                fixture.origin.clone(),
            ),
        ])
        .unwrap(),
    )
    .unwrap();
    let mut authority = fixture.authority();
    authority.expected_mapping = Some(fixture.identity_mapping());
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Ok(())
    );
    template.type_parameters =
        CanonicalBinderUseListV1::try_new(vec![binder(0, 1), binder(0, 1)]).unwrap();
    assert!(matches!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::DefinitionRoot(
                DefaultTemplateRootSemanticValidationError::InheritedRelation(
                    AuthorityError::Inherited
                )
            )
        )
    ));
}
