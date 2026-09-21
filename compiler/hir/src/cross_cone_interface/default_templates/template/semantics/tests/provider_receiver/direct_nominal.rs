use super::*;
use scoop_identity::PersistentFunctionId;

#[test]
fn generic_member_receiver_uses_the_declared_host_frame() {
    for own_arity in [0, 1] {
        let nominal = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("GenericOwner"),
            SourceNominalKind::Class,
            1,
        );
        let nominal = PersistentGenericTypeId::from_source_declaration(&nominal).unwrap();
        let owner =
            PublicDeclarationOwnerV1::Nominal(crate::SourceNominalId::GenericTemplate(nominal));
        let parameters = vec![binder(0, 0); 2];
        let declaration = SourceDeclarationKey::function(
            owned_site(DefinitionOwnerAtom::GenericType(nominal)),
            identifier("pick"),
            own_arity,
            None,
            parameters.clone(),
        );
        let root = if own_arity == 0 {
            PersistentLexicalRootV1::Function(
                PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
            )
        } else {
            PersistentLexicalRootV1::GenericFunction(
                PersistentGenericFunctionId::from_source_declaration(&declaration).unwrap(),
            )
        };
        let mut fixture = Fixture::new();
        fixture.key = ExportDefaultTemplateKeyV1::new(root.declaration(), 1);
        fixture.provider = root;
        fixture.identity =
            CallableDeclarationIdentityShapeV1::new(owner, own_arity, 1, None, parameters.clone());
        let callable = CallableInterfaceRecordV1::try_new(
            root.declaration(),
            owner,
            binders(own_arity),
            None,
            source_shapes(parameters),
            binder(0, 0),
            effects(Effect::Ordinary),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap();
        let source = CallableSourceInterfaceV1::try_new(
            root.declaration(),
            CanonicalCallableSourceParametersV1::try_new(vec![
                CallableSourceParameterV1::new(
                    identifier("p0"),
                    binder(0, 0),
                    CallableParameterCallingV1::Required,
                    fixture.origin.clone(),
                ),
                CallableSourceParameterV1::new(
                    identifier("p1"),
                    binder(0, 0),
                    CallableParameterCallingV1::Default {
                        template: fixture.key,
                    },
                    fixture.origin.clone(),
                ),
            ])
            .unwrap(),
        )
        .unwrap();
        let receiver = SignatureTypeKey::NominalApplication {
            origin: nominal,
            arguments: NonEmptyVec::from_first(binder(own_arity, 0), []),
        };
        let mapping = if own_arity == 0 {
            vec![binder(0, 0)]
        } else {
            vec![binder(1, 0), binder(0, 0)]
        };
        let mut template = fixture.template(
            binder(0, 0),
            CanonicalBinderUseListV1::try_new(mapping).unwrap(),
            CanonicalBooleanV1::False,
            CanonicalBooleanV1::False,
        );
        set_receiver(&mut template, receiver.clone());
        let mut authority = fixture.authority();
        authority.nominals.push((
            crate::SourceNominalId::GenericTemplate(nominal),
            PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
        ));
        authority.provider_shape = DefaultTemplateProviderShapeV1::try_new(1, own_arity).unwrap();
        authority.provider_receiver = Some(receiver);
        assert_eq!(
            template.validate_contract_semantics(&callable, &source, &mut authority),
            Ok(())
        );
        // Corrupt the independent receiver and candidate together: their
        // agreement cannot override the declared host's identity application.
        let wrong = SignatureTypeKey::NominalApplication {
            origin: nominal,
            arguments: NonEmptyVec::from_first(
                SignatureTypeKey::Tuple(NonEmptyVec::from_first(binder(0, 0), [])),
                [],
            ),
        };
        set_receiver(&mut template, wrong.clone());
        authority.provider_receiver = Some(wrong);
        assert!(matches!(
            template.validate_contract_semantics(&callable, &source, &mut authority),
            Err(ExportDefaultTemplateContractSemanticValidationError::ProviderOwnerReceiver)
        ));
    }
}
