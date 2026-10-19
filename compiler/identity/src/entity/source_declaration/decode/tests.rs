use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, decode_canonical, encode};

use super::{DecodedSourceDeclarationKey, SourceDeclarationResolutionError};
use crate::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DecodedCborIdentityRecord, DefinitionOwnerChain, NonEmptyVec, PackagePath,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExtensionPropertyId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationKind, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver;

trait TestId {
    fn expected() -> Self;
}

macro_rules! test_identity {
    ($id:ty) => {
        impl TestId for $id {
            fn expected() -> Self {
                Self([7; 32])
            }
        }

        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::SINGLE_FILE)
            .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
    }
}

test_identity!(PersistentTypeId);
test_identity!(PersistentGenericTypeId);
test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyId);
test_identity!(PersistentExtensionPropertyId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentEnumVariantId);

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::SourceScoped(SourceIdentity::single_file()),
    )
    .unwrap()
}

fn name(text: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(text).unwrap()
}

fn signature() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(PersistentTypeId::expected())
}

#[test]
fn all_source_declaration_shapes_round_trip_and_resolve() {
    let mut keys = Vec::new();
    for (index, kind) in [
        SourceNominalKind::Class,
        SourceNominalKind::Interface,
        SourceNominalKind::Struct,
        SourceNominalKind::Enum,
        SourceNominalKind::Object,
        SourceNominalKind::AnnotationClass,
    ]
    .into_iter()
    .enumerate()
    {
        keys.push(SourceDeclarationKey::nominal(
            site(),
            name(&format!("Nominal{index}")),
            kind,
            index as u32,
        ));
    }
    keys.push(SourceDeclarationKey::function(
        site(),
        name("function"),
        1,
        Some(signature()),
        vec![SignatureTypeKey::NominalApplication {
            origin: PersistentGenericTypeId::expected(),
            arguments: NonEmptyVec::from_first(signature(), []),
        }],
    ));
    keys.push(SourceDeclarationKey::constructor(site(), vec![signature()]));
    keys.push(SourceDeclarationKey::property(site(), name("property")));
    keys.push(SourceDeclarationKey::extension_property(
        site(),
        name("extension"),
        1,
        signature(),
    ));
    keys.push(SourceDeclarationKey::type_alias(site(), name("Alias")));

    for key in keys {
        let bytes = encode(&key).unwrap();
        let decoded = decode_canonical::<DecodedSourceDeclarationKey>(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }
}

#[test]
fn function_conditions_normalize_order_and_distinguish_declaration_identity() {
    use crate::SignatureConditionKey;
    let parameter = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let conditions = vec![
        SignatureConditionKey::Value {
            subject: parameter.clone(),
        },
        SignatureConditionKey::ExcludedInterface {
            subject: parameter.clone(),
            bound: signature(),
        },
    ];
    let key = |conditions| {
        SourceDeclarationKey::constrained_function(
            site(),
            name("select"),
            1,
            None,
            vec![parameter.clone()],
            conditions,
        )
    };
    let record = |key| CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(key).unwrap();
    let expected = record(key(conditions.clone()));
    assert_eq!(
        expected,
        record(key(conditions.into_iter().rev().collect()))
    );
    assert_ne!(expected.id(), record(key(Vec::new())).id());
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentGenericFunctionId, DecodedSourceDeclarationKey>,
    >(&encode(&expected).unwrap())
    .unwrap();
    assert_eq!(
        decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
        expected
    );
}

#[test]
fn source_declaration_record_resolves_before_recomputing_its_id() {
    let key = SourceDeclarationKey::function(site(), name("work"), 0, None, vec![signature()]);
    let record = CborIdentityRecord::<PersistentFunctionId, _>::from_key(key).unwrap();
    let bytes = encode(&record).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentFunctionId, DecodedSourceDeclarationKey>,
    >(&bytes)
    .unwrap();
    let resolved = decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap();
    assert_eq!(resolved, record);
}

#[test]
fn declaration_kind_and_duplicate_signature_must_match() {
    let key = SourceDeclarationKey::function(site(), name("work"), 0, None, vec![]);
    let bytes = encode(&key).unwrap();
    let mut decoded = decode_canonical::<DecodedSourceDeclarationKey>(&bytes).unwrap();
    decoded.declaration_kind = SourceDeclarationKind::Property;
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(SourceDeclarationResolutionError::Shape(
            SourceDeclarationKind::Property
        ))
    ));
}

#[derive(Debug, Eq, PartialEq)]
struct DecodedKind(SourceDeclarationKind);

impl WireEncode for DecodedKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireDecode for DecodedKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        super::decode_declaration_kind(decoder).map(Self)
    }
}

#[test]
fn declaration_kind_decoder_rejects_unknown_tags() {
    let error = decode_canonical::<DecodedKind>(b"\x0c").unwrap_err();
    assert_eq!(
        error.kind(),
        &scoop_wire::WireErrorKind::UnknownTag { tag: 12 }
    );
}
