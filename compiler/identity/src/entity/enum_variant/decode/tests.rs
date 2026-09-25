use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{
    DecodedEnumVariantFieldKey, DecodedEnumVariantFieldSelector, DecodedEnumVariantIdentityKey,
    EnumVariantResolutionError,
};
use crate::{
    CanonicalIdentifier, CborIdentityRecord, DecodedCborIdentityRecord, DecodedPersistentId,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityError,
    EnumVariantIdentityKey, GeneratedEnumVariantRole, GeneratedNominalKey, PackagePath,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentKeyResolver, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver {
    source: Vec<SourceDeclarationKey>,
    generated: Vec<GeneratedNominalKey>,
    variants: Vec<PersistentEnumVariantId>,
}

impl PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        self.source
            .iter()
            .find(|key| {
                PersistentTypeId::from_source_declaration(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        self.source
            .iter()
            .find(|key| {
                PersistentGenericTypeId::from_source_declaration(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentTypeId, GeneratedNominalKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<std::sync::Arc<GeneratedNominalKey>, Self::Error> {
        self.generated
            .iter()
            .find(|key| {
                PersistentTypeId::from_generated_key(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

impl PersistentIdResolver<PersistentEnumVariantId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        self.variants
            .iter()
            .copied()
            .find(|variant| variant.as_array() == id.as_array())
            .ok_or(ResolutionError)
    }
}

#[test]
fn source_and_generated_variant_records_resolve_through_owner_keys() {
    let source = source_enum("SourceEnum", 1);
    let exact = PersistentExactTypeId([7; 32]);
    let generated = vec![
        GeneratedNominalKey::CoroutineStep { result: exact },
        GeneratedNominalKey::CoroutineSlot { value: exact },
    ];
    let keys = vec![
        EnumVariantIdentityKey::source(&source, CanonicalIdentifier::new("SourceValue").unwrap())
            .unwrap(),
        EnumVariantIdentityKey::generated(
            &generated[0],
            GeneratedEnumVariantRole::CoroutineStepCompleted,
        )
        .unwrap(),
        EnumVariantIdentityKey::generated(
            &generated[0],
            GeneratedEnumVariantRole::CoroutineStepSuspended,
        )
        .unwrap(),
        EnumVariantIdentityKey::generated(
            &generated[1],
            GeneratedEnumVariantRole::CoroutineSlotEmpty,
        )
        .unwrap(),
        EnumVariantIdentityKey::generated(
            &generated[1],
            GeneratedEnumVariantRole::CoroutineSlotValue,
        )
        .unwrap(),
    ];
    let mut resolver = Resolver {
        source: vec![source],
        generated,
        variants: vec![],
    };

    for key in keys {
        let record = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentEnumVariantId, DecodedEnumVariantIdentityKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn variant_resolution_rejects_a_role_that_does_not_match_its_owner() {
    let owner = GeneratedNominalKey::BoxedValue {
        payload: PersistentExactTypeId([7; 32]),
    };
    let owner_id = PersistentTypeId::from_generated_key(&owner).unwrap();
    let decoded = DecodedEnumVariantIdentityKey::Generated {
        owner: DecodedPersistentId::from_unvalidated_bytes(*owner_id.as_array()),
        role: GeneratedEnumVariantRole::CoroutineSlotEmpty,
    };
    let mut resolver = Resolver {
        source: vec![],
        generated: vec![owner],
        variants: vec![],
    };

    assert_eq!(
        decoded.resolve(&mut resolver),
        Err(EnumVariantResolutionError::Key(
            EnumVariantIdentityError::GeneratedRoleMismatch
        ))
    );
}

#[test]
fn named_and_positional_variant_field_records_resolve() {
    let owner = source_enum("Fields", 0);
    let variant_key =
        EnumVariantIdentityKey::source(&owner, CanonicalIdentifier::new("Value").unwrap()).unwrap();
    let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
    let keys = [
        EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Named(CanonicalIdentifier::new("payload").unwrap()),
        ),
        EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 3,
            },
        ),
    ];
    let mut resolver = Resolver {
        source: vec![],
        generated: vec![],
        variants: vec![variant],
    };

    for key in keys {
        let record = CborIdentityRecord::<PersistentEnumVariantFieldId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentEnumVariantFieldId, DecodedEnumVariantFieldKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn enum_variant_decoder_rejects_unknown_tags() {
    let outer = decode_canonical::<DecodedEnumVariantIdentityKey>(b"\xa3\x00\x03\x01\x00\x02\x00")
        .unwrap_err();
    assert_eq!(outer.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let selector =
        decode_canonical::<DecodedEnumVariantFieldSelector>(b"\xa2\x00\x03\x01\x00").unwrap_err();
    assert_eq!(selector.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let role = decode_canonical::<GeneratedEnumVariantRole>(b"\x05").unwrap_err();
    assert_eq!(role.kind(), &WireErrorKind::UnknownTag { tag: 5 });
}

fn source_enum(name: &str, type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            crate::ConeIdentity::CORE,
            PackagePath::root(),
            crate::DefinitionOwnerChain::top_level(),
            crate::DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Enum,
        type_parameter_count,
    )
}
