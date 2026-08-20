use super::*;

#[test]
fn protocol_constructor_adapters_replay_actual_source_and_result_for_each_provider() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let owner: CborIdentityRecord<PersistentTypeId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site_at(origin),
                CanonicalIdentifier::new("Owner").unwrap(),
                SourceNominalKind::Class,
                0,
            ))
            .unwrap();
        let constructor: CborIdentityRecord<PersistentConstructorId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
                SourceDeclarationSite::new(
                    origin,
                    PackagePath::root(),
                    DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                        owner.id(),
                    )]),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                vec![],
            ))
            .unwrap();
        let adapter: CborIdentityRecord<PersistentGeneratedCallableId, _> =
            CborIdentityRecord::from_key(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor: constructor.id(),
            })
            .unwrap();
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_types(vec![
                owner.clone(),
                CoreBuiltinNominal::Unit.identity_record(),
            ])
            .unwrap();
        foundation
            .set_constructors(vec![constructor.clone()])
            .unwrap();
        foundation
            .set_generated_callables(vec![adapter.clone()])
            .unwrap();
        foundation
            .set_definition_origins(vec![
                origin_record_at(
                    origin,
                    DefinitionOriginSubject::Constructor(constructor.id()),
                ),
                origin_record_at(
                    origin,
                    DefinitionOriginSubject::GeneratedCallable(adapter.id()),
                ),
            ])
            .unwrap();
        for definition in [
            CoreProtocolCallableDefinitionV1::Constructor(constructor.id()),
            CoreProtocolCallableDefinitionV1::GeneratedCallable(adapter.id()),
        ] {
            let mut callable = CoreProtocolCallableV1::for_test(
                definition,
                SignatureCallableShape::new(
                    Effect::Ordinary,
                    None,
                    vec![],
                    SignatureTypeKey::Nominal(owner.id()),
                ),
            );
            assert_eq!(
                decode(&callable).validate_against(&foundation),
                Ok(callable.clone())
            );
            callable.signature = SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                vec![],
                SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            );
            assert_eq!(
                decode(&callable).validate_against(&foundation),
                Err(CoreProtocolCallableValidationError::ConstructorResultMismatch(definition))
            );
        }
        let callable = CoreProtocolCallableV1::for_test(
            CoreProtocolCallableDefinitionV1::GeneratedCallable(adapter.id()),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                vec![],
                SignatureTypeKey::Nominal(owner.id()),
            ),
        );
        foundation.set_constructors(vec![]).unwrap();
        assert_eq!(
            decode(&callable).validate_against(&foundation),
            Err(CoreProtocolCallableValidationError::MissingAdapterSource(
                callable.definition()
            ))
        );
    }
}

fn decode(callable: &CoreProtocolCallableV1) -> DecodedCoreProtocolCallableV1 {
    decode_canonical(&encode(callable).unwrap()).unwrap()
}
