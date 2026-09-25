use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{decode_id_variant, decode_sum_header, decode_value_variant, encode_tag, unknown_tag};
use crate::{
    ConeIdentity, DecodedCallableOwner, DecodedNominalDeclarationOwner, DecodedNominalOwner,
    DecodedPersistentId, DecodedPropertyOwner, InitializationUnitKey, MainCallableBodyId,
    PersistentCallableBodyId, PersistentEnumVariantId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentPropertyId, PersistentTypeId, RuntimeIdentityError, StaticStorageKey, StorageRole,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DecodedDefinitionOwner {
    Nominal(DecodedNominalOwner),
    Callable(DecodedCallableOwner),
    Property(DecodedPropertyOwner),
    Field(DecodedPersistentId<PersistentFieldId>),
    EnumVariant(DecodedPersistentId<PersistentEnumVariantId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
    RootEntry {
        root_cone: DecodedPersistentId<ConeIdentity>,
        main: DecodedPersistentId<PersistentCallableBodyId>,
    },
}

impl WireEncode for DecodedDefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(owner) => encode_value_sum(encoder, 1, owner),
            Self::Callable(owner) => encode_value_sum(encoder, 2, owner),
            Self::Property(owner) => encode_value_sum(encoder, 3, owner),
            Self::Field(id) => encode_value_sum(encoder, 4, id),
            Self::EnumVariant(id) => encode_value_sum(encoder, 5, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 6, id),
            Self::RootEntry { root_cone, main } => {
                encoder.map(3)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                root_cone.encode(encoder)?;
                encoder.field(2)?;
                main.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDefinitionOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_value_variant(decoder, fields, DecodedNominalOwner::decode, Self::Nominal),
            2 => decode_value_variant(
                decoder,
                fields,
                DecodedCallableOwner::decode,
                Self::Callable,
            ),
            3 => decode_value_variant(
                decoder,
                fields,
                DecodedPropertyOwner::decode,
                Self::Property,
            ),
            4 => decode_id_variant(decoder, fields, Self::Field),
            5 => decode_id_variant(decoder, fields, Self::EnumVariant),
            6 => decode_id_variant(decoder, fields, Self::InitializationUnit),
            7 => {
                super::expect_sum_length(decoder, fields, 3)?;
                Ok(Self::RootEntry {
                    root_cone: decoder.field(1, DecodedPersistentId::decode)?,
                    main: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedStaticStorageKey {
    owner: DecodedDefinitionOwner,
    role: StorageRole,
}

impl DecodedStaticStorageKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<StaticStorageKey, StaticStorageResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
            + PersistentKeyResolver<PersistentInitializationUnitId, InitializationUnitKey, Error = E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentCallableBodyId, Error = E>,
    {
        match (self.owner, self.role) {
            (DecodedDefinitionOwner::Property(owner), StorageRole::PropertyBacking) => owner
                .resolve(resolver)
                .map(StaticStorageKey::property_backing)
                .map_err(StaticStorageResolutionError::Reference),
            (DecodedDefinitionOwner::Property(owner), StorageRole::PropertyDelegate) => owner
                .resolve(resolver)
                .map(StaticStorageKey::property_delegate)
                .map_err(StaticStorageResolutionError::Reference),
            (DecodedDefinitionOwner::Property(owner), StorageRole::StaticPlaceToken) => owner
                .resolve(resolver)
                .map(StaticStorageKey::static_place_for_property)
                .map_err(StaticStorageResolutionError::Reference),
            (
                DecodedDefinitionOwner::Nominal(DecodedNominalOwner::Declaration(
                    DecodedNominalDeclarationOwner::Concrete(owner),
                )),
                StorageRole::SingletonPublishedRoot,
            ) => resolver
                .resolve(owner)
                .map(StaticStorageKey::singleton_published_root)
                .map_err(StaticStorageResolutionError::Reference),
            (
                DecodedDefinitionOwner::InitializationUnit(unit),
                StorageRole::InitializationFailureRoot,
            ) => resolver
                .resolve(unit)
                .map(StaticStorageKey::initialization_failure_root)
                .map_err(StaticStorageResolutionError::Reference),
            (
                DecodedDefinitionOwner::RootEntry { root_cone, main },
                StorageRole::RootEntryFailureRoot,
            ) => {
                let root_cone = resolver
                    .resolve(root_cone)
                    .map_err(StaticStorageResolutionError::Reference)?;
                let main = resolver
                    .resolve(main)
                    .map(MainCallableBodyId::from_body)
                    .map_err(StaticStorageResolutionError::Reference)?;
                Ok(StaticStorageKey::root_entry_failure_root(root_cone, main))
            }
            (DecodedDefinitionOwner::InitializationUnit(unit), StorageRole::PropertyBacking) => {
                let unit = resolver
                    .resolve_key(unit)
                    .map_err(StaticStorageResolutionError::Reference)?;
                StaticStorageKey::delegated_application_backing(&unit)
                    .map_err(StaticStorageResolutionError::Shape)
            }
            (DecodedDefinitionOwner::InitializationUnit(unit), StorageRole::PropertyDelegate) => {
                let unit = resolver
                    .resolve_key(unit)
                    .map_err(StaticStorageResolutionError::Reference)?;
                StaticStorageKey::delegated_application_delegate(&unit)
                    .map_err(StaticStorageResolutionError::Shape)
            }
            (DecodedDefinitionOwner::InitializationUnit(unit), StorageRole::StaticPlaceToken) => {
                let unit = resolver
                    .resolve_key(unit)
                    .map_err(StaticStorageResolutionError::Reference)?;
                StaticStorageKey::static_place_for_delegated_application(&unit)
                    .map_err(StaticStorageResolutionError::Shape)
            }
            (_, role) => Err(StaticStorageResolutionError::InvalidOwnerRole(role)),
        }
    }
}

impl WireEncode for DecodedStaticStorageKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedStaticStorageKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedDefinitionOwner::decode)?,
            role: decoder.field(2, StorageRole::decode)?,
        })
    }
}

impl WireDecode for StorageRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::PropertyBacking),
            2 => Ok(Self::PropertyDelegate),
            3 => Ok(Self::SingletonPublishedRoot),
            4 => Ok(Self::InitializationFailureRoot),
            5 => Ok(Self::RootEntryFailureRoot),
            6 => Ok(Self::StaticPlaceToken),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StaticStorageResolutionError<E> {
    Reference(E),
    InvalidOwnerRole(StorageRole),
    Shape(RuntimeIdentityError),
}

impl<E: fmt::Display> fmt::Display for StaticStorageResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::InvalidOwnerRole(role) => {
                write!(
                    formatter,
                    "invalid definition owner for storage role {role:?}"
                )
            }
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StaticStorageResolutionError<E> {}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
