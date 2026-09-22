use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerAtom,
    DefinitionOwnerChain, GeneratedCallableKey, NormalizedSourcePath, PackagePath,
    PersistentConstructorId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn protocol_callable_roundtrips_and_replays_the_source_signature() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site_at(origin),
                CanonicalIdentifier::new("probe").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let callable = CoreProtocolCallableV1 {
            definition: CoreProtocolCallableDefinitionV1::Function(function.id()),
            signature: SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Nominal(unit.id()),
            ),
        };
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(vec![unit]).unwrap();
        foundation.set_functions(vec![function.clone()]).unwrap();
        foundation
            .set_definition_origins(vec![origin_record_at(
                origin,
                DefinitionOriginSubject::Function(function.id()),
            )])
            .unwrap();

        let bytes = encode(&callable).unwrap();
        let decoded: DecodedCoreProtocolCallableV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.validate_against(&foundation), Ok(callable));
    }
}

#[test]
fn protocol_callable_reader_rejects_unknown_open_and_wrong_signature_values() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        for bytes in [vec![0xa1], vec![0xa3], vec![0xa2, 0x03, 0x00, 0x02, 0x00]] {
            assert!(
                decode_canonical::<DecodedCoreProtocolCallableV1>(&bytes, DecodeLimits::default())
                    .is_err()
            );
        }

        let plain: CborIdentityRecord<PersistentFunctionId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site_at(origin),
                CanonicalIdentifier::new("plain").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let generic: CborIdentityRecord<PersistentGenericFunctionId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site_at(origin),
                CanonicalIdentifier::new("generic").unwrap(),
                1,
                None,
                Vec::new(),
            ))
            .unwrap();
        let constructor: CborIdentityRecord<PersistentConstructorId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
                SourceDeclarationSite::new(
                    origin,
                    PackagePath::root(),
                    DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                        CoreBuiltinNominal::Unit.identity_record().id(),
                    )]),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                Vec::new(),
            ))
            .unwrap();
        let generated: CborIdentityRecord<PersistentGeneratedCallableId, _> =
            CborIdentityRecord::from_key(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor: constructor.id(),
            })
            .unwrap();
        for definition in [
            CoreProtocolCallableDefinitionV1::Function(plain.id()),
            CoreProtocolCallableDefinitionV1::GenericFunction(generic.id()),
            CoreProtocolCallableDefinitionV1::Constructor(constructor.id()),
            CoreProtocolCallableDefinitionV1::GeneratedCallable(generated.id()),
        ] {
            let bytes = encode(&definition).unwrap();
            let decoded: DecodedCoreProtocolCallableDefinitionV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
        }

        let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site_at(origin),
                CanonicalIdentifier::new("probe").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let wrong = CoreProtocolCallableV1 {
            definition: CoreProtocolCallableDefinitionV1::Function(function.id()),
            signature: SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                vec![SignatureTypeKey::Nominal(unit.id())],
                SignatureTypeKey::Nominal(unit.id()),
            ),
        };
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(vec![unit]).unwrap();
        foundation.set_functions(vec![function.clone()]).unwrap();
        foundation
            .set_definition_origins(vec![origin_record_at(
                origin,
                DefinitionOriginSubject::Function(function.id()),
            )])
            .unwrap();
        let decoded: DecodedCoreProtocolCallableV1 =
            decode_canonical(&encode(&wrong).unwrap(), DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded.validate_against(&foundation),
            Err(
                CoreProtocolCallableValidationError::SourceSignatureMismatch(
                    CoreProtocolCallableDefinitionV1::Function(function.id())
                )
            )
        );
    }
}

#[test]
fn protocol_callable_reader_replays_owner_and_callable_binder_groups() {
    for origin in [
        ConeIdentity::CORE,
        crate::core_protocol_test_support::ordinary_origin(),
    ] {
        let owner: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site_at(origin),
                CanonicalIdentifier::new("Owner").unwrap(),
                SourceNominalKind::Interface,
                1,
            ))
            .unwrap();
        let owner_parameter = SignatureTypeKey::Binder { depth: 1, index: 0 };
        let own_parameter = SignatureTypeKey::Binder { depth: 0, index: 0 };
        let function: CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    origin,
                    PackagePath::root(),
                    DefinitionOwnerChain::from_outer_to_inner(vec![
                        DefinitionOwnerAtom::GenericType(owner.id()),
                    ]),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("map").unwrap(),
                1,
                None,
                vec![owner_parameter.clone()],
            ))
            .unwrap();
        let callable = CoreProtocolCallableV1 {
            definition: CoreProtocolCallableDefinitionV1::GenericFunction(function.id()),
            signature: SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                vec![owner_parameter],
                own_parameter,
            ),
        };
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_generic_types(vec![owner.clone()]).unwrap();
        foundation
            .set_generic_functions(vec![function.clone()])
            .unwrap();
        foundation
            .set_definition_origins(vec![
                origin_record_at(origin, DefinitionOriginSubject::GenericType(owner.id())),
                origin_record_at(
                    origin,
                    DefinitionOriginSubject::GenericFunction(function.id()),
                ),
            ])
            .unwrap();

        let decoded: DecodedCoreProtocolCallableV1 =
            decode_canonical(&encode(&callable).unwrap(), DecodeLimits::default()).unwrap();
        assert_eq!(decoded.validate_against(&foundation), Ok(callable));
    }
}

fn site_at(origin: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn origin_record_at(
    origin: ConeIdentity,
    subject: DefinitionOriginSubject,
) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        origin,
        NormalizedSourcePath::new("src/protocol.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    DefinitionOriginRecord::new(
        subject,
        DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap(),
    )
}

mod adapters;
