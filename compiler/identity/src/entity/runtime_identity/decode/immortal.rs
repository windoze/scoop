use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{decode_id_variant, decode_sum_header, decode_value_variant, encode_tag, unknown_tag};
use crate::{
    DecodedCallableMaterialization, DecodedPersistentId, DecodedPropertyOwner, ImmortalObjectKey,
    ImmortalObjectOwner, ImmortalObjectRole, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentPropertyAccessorId, PersistentPropertyId,
    StructuralDefinitionPath,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedImmortalObjectOwner {
    Callable(DecodedCallableMaterialization),
    Property(DecodedPropertyOwner),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
}

impl DecodedImmortalObjectOwner {
    fn resolve<R, E>(self, resolver: &mut R) -> Result<ImmortalObjectOwner, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<crate::PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>,
    {
        match self {
            Self::Callable(owner) => owner.resolve(resolver).map(ImmortalObjectOwner::Callable),
            Self::Property(owner) => owner.resolve(resolver).map(ImmortalObjectOwner::Property),
            Self::InitializationUnit(id) => resolver
                .resolve(id)
                .map(ImmortalObjectOwner::InitializationUnit),
        }
    }
}

impl WireEncode for DecodedImmortalObjectOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(owner) => encode_value_sum(encoder, 1, owner),
            Self::Property(owner) => encode_value_sum(encoder, 2, owner),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

impl WireDecode for DecodedImmortalObjectOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_value_variant(
                decoder,
                fields,
                DecodedCallableMaterialization::decode,
                Self::Callable,
            ),
            2 => decode_value_variant(
                decoder,
                fields,
                DecodedPropertyOwner::decode,
                Self::Property,
            ),
            3 => decode_id_variant(decoder, fields, Self::InitializationUnit),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for ImmortalObjectRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::StringConstant),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedImmortalObjectKey {
    owner: DecodedImmortalObjectOwner,
    role: ImmortalObjectRole,
    path: StructuralDefinitionPath,
}

impl DecodedImmortalObjectKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<ImmortalObjectKey, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<crate::PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>,
    {
        let owner = self.owner.resolve(resolver)?;
        match self.role {
            ImmortalObjectRole::StringConstant => {
                Ok(ImmortalObjectKey::string_constant(owner, self.path))
            }
        }
    }
}

impl WireEncode for DecodedImmortalObjectKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.path.encode(encoder)
    }
}

impl WireDecode for DecodedImmortalObjectKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedImmortalObjectOwner::decode)?,
            role: decoder.field(2, ImmortalObjectRole::decode)?,
            path: decoder.field(3, StructuralDefinitionPath::decode)?,
        })
    }
}

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
