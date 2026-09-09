use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityError,
    EnumVariantIdentityKey, GeneratedEnumVariantRole,
};
use crate::{
    CanonicalIdentifierError, DecodedCanonicalIdentifier, DecodedNominalDeclarationOwner,
    DecodedPersistentId, GeneratedNominalKey, PersistentEnumVariantId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentKeyResolver, PersistentTypeId, SourceDeclarationKey,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedEnumVariantIdentityKey {
    Source {
        owner: DecodedNominalDeclarationOwner,
        name: DecodedCanonicalIdentifier,
    },
    Generated {
        owner: DecodedPersistentId<PersistentTypeId>,
        role: GeneratedEnumVariantRole,
    },
}

impl DecodedEnumVariantIdentityKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumVariantIdentityKey, EnumVariantResolutionError<E>>
    where
        R: PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentTypeId, GeneratedNominalKey, Error = E>,
    {
        match self {
            Self::Source { owner, name } => {
                let owner = resolve_source_owner(owner, resolver)?;
                let name = name.validate().map_err(EnumVariantResolutionError::Name)?;
                EnumVariantIdentityKey::source(&owner, name)
                    .map_err(EnumVariantResolutionError::Key)
            }
            Self::Generated { owner, role } => {
                let owner = resolver
                    .resolve_key(owner)
                    .map_err(EnumVariantResolutionError::Reference)?;
                EnumVariantIdentityKey::generated(&owner, role)
                    .map_err(EnumVariantResolutionError::Key)
            }
        }
    }
}

impl WireEncode for DecodedEnumVariantIdentityKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source { owner, name } => encode_two_value_sum(encoder, 1, owner, name),
            Self::Generated { owner, role } => encode_two_value_sum(encoder, 2, owner, role),
        }
    }
}

impl WireDecode for DecodedEnumVariantIdentityKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::Source {
                owner: decoder.field(1, DecodedNominalDeclarationOwner::decode)?,
                name: decoder.field(2, DecodedCanonicalIdentifier::decode)?,
            }),
            2 => Ok(Self::Generated {
                owner: decoder.field(1, DecodedPersistentId::decode)?,
                role: decoder.field(2, GeneratedEnumVariantRole::decode)?,
            }),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedEnumVariantFieldSelector {
    Named(DecodedCanonicalIdentifier),
    Positional { declaration_index: u32 },
}

impl DecodedEnumVariantFieldSelector {
    pub fn validate(self) -> Result<EnumVariantFieldSelector, CanonicalIdentifierError> {
        match self {
            Self::Named(name) => name.validate().map(EnumVariantFieldSelector::Named),
            Self::Positional { declaration_index } => {
                Ok(EnumVariantFieldSelector::Positional { declaration_index })
            }
        }
    }
}

impl WireEncode for DecodedEnumVariantFieldSelector {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named(name) => encode_value_sum(encoder, 1, name),
            Self::Positional { declaration_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*declaration_index))
            }
        }
    }
}

impl WireDecode for DecodedEnumVariantFieldSelector {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedCanonicalIdentifier::decode)
                .map(Self::Named),
            2 => decoder
                .field(1, Decoder::u32)
                .map(|declaration_index| Self::Positional { declaration_index }),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumVariantFieldKey {
    variant: DecodedPersistentId<PersistentEnumVariantId>,
    selector: DecodedEnumVariantFieldSelector,
}

impl DecodedEnumVariantFieldKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumVariantFieldKey, EnumVariantFieldResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentEnumVariantId, Error = E>,
    {
        let variant = resolver
            .resolve(self.variant)
            .map_err(EnumVariantFieldResolutionError::Reference)?;
        let selector = self
            .selector
            .validate()
            .map_err(EnumVariantFieldResolutionError::Name)?;
        Ok(EnumVariantFieldKey::new(variant, selector))
    }
}

impl WireEncode for DecodedEnumVariantFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.selector.encode(encoder)
    }
}

impl WireDecode for DecodedEnumVariantFieldKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            selector: decoder.field(2, DecodedEnumVariantFieldSelector::decode)?,
        })
    }
}

impl WireDecode for GeneratedEnumVariantRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::CoroutineStepCompleted),
            2 => Ok(Self::CoroutineStepSuspended),
            3 => Ok(Self::CoroutineSlotEmpty),
            4 => Ok(Self::CoroutineSlotValue),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantResolutionError<E> {
    Reference(E),
    Name(CanonicalIdentifierError),
    Key(EnumVariantIdentityError),
}

impl<E: fmt::Display> fmt::Display for EnumVariantResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Name(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumVariantResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantFieldResolutionError<E> {
    Reference(E),
    Name(CanonicalIdentifierError),
}

impl<E: fmt::Display> fmt::Display for EnumVariantFieldResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Name(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumVariantFieldResolutionError<E> {}

fn resolve_source_owner<R, E>(
    owner: DecodedNominalDeclarationOwner,
    resolver: &mut R,
) -> Result<SourceDeclarationKey, EnumVariantResolutionError<E>>
where
    R: PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>,
{
    match owner {
        DecodedNominalDeclarationOwner::Concrete(id) => resolver.resolve_key(id),
        DecodedNominalDeclarationOwner::GenericTemplate(id) => resolver.resolve_key(id),
    }
    .map_err(EnumVariantResolutionError::Reference)
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

#[cfg(test)]
mod tests;
