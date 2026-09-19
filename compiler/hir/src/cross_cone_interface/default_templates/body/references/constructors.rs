mod resolution_nodes;

use std::fmt;

mod resources;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentConstructorId, PersistentEnumVariantId,
    PersistentGeneratedCallableId, PersistentIdResolver, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::SignatureTypeReferenceResolver;

/// Stable identity of either a source class constructor or its generated
/// zero-argument adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultClassConstructorIdV1 {
    Source(PersistentConstructorId),
    Generated(PersistentGeneratedCallableId),
}

impl WireEncode for DefaultClassConstructorIdV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Source(_) => 1,
            Self::Generated(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Source(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDefaultClassConstructorIdV1 {
    Source(DecodedPersistentId<PersistentConstructorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedDefaultClassConstructorIdV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefaultClassConstructorIdV1, E>
    where
        R: DefaultClassConstructorIdResolver<E>,
    {
        match self {
            Self::Source(id) => resolver
                .resolve(id)
                .map(DefaultClassConstructorIdV1::Source),
            Self::Generated(id) => resolver
                .resolve(id)
                .map(DefaultClassConstructorIdV1::Generated),
        }
    }
}

impl WireEncode for DecodedDefaultClassConstructorIdV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Source(_) => 1,
            Self::Generated(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Source(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultClassConstructorIdV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Source),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

/// Constructor reference with the complete applied owner type needed by the
/// consumer to rebuild its local application arena.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultConstructorRefV1 {
    Struct {
        declaration: PersistentConstructorId,
        owner_type: SignatureTypeKey,
    },
    Class {
        declaration: DefaultClassConstructorIdV1,
        owner_type: SignatureTypeKey,
    },
    Variant {
        declaration: PersistentEnumVariantId,
        owner_type: SignatureTypeKey,
    },
}

impl DefaultConstructorRefV1 {
    pub const fn owner_type(&self) -> &SignatureTypeKey {
        match self {
            Self::Struct { owner_type, .. }
            | Self::Class { owner_type, .. }
            | Self::Variant { owner_type, .. } => owner_type,
        }
    }
}

impl WireEncode for DefaultConstructorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Struct { .. } => 1,
            Self::Class { .. } => 2,
            Self::Variant { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Struct { declaration, .. } => declaration.encode(encoder)?,
            Self::Class { declaration, .. } => declaration.encode(encoder)?,
            Self::Variant { declaration, .. } => declaration.encode(encoder)?,
        }
        encoder.field(2)?;
        self.owner_type().encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultConstructorRefV1 {
    Struct {
        declaration: DecodedPersistentId<PersistentConstructorId>,
        owner_type: DecodedSignatureTypeKey,
    },
    Class {
        declaration: DecodedDefaultClassConstructorIdV1,
        owner_type: DecodedSignatureTypeKey,
    },
    Variant {
        declaration: DecodedPersistentId<PersistentEnumVariantId>,
        owner_type: DecodedSignatureTypeKey,
    },
}

impl DecodedDefaultConstructorRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultConstructorRefV1, DefaultConstructorRefResolutionError<E>>
    where
        R: DefaultConstructorReferenceResolver<E>,
    {
        match self {
            Self::Struct {
                declaration,
                owner_type,
            } => Ok(DefaultConstructorRefV1::Struct {
                declaration: resolver
                    .resolve(declaration)
                    .map_err(DefaultConstructorRefResolutionError::StructDeclaration)?,
                owner_type: owner_type
                    .resolve(resolver)
                    .map_err(DefaultConstructorRefResolutionError::OwnerType)?,
            }),
            Self::Class {
                declaration,
                owner_type,
            } => Ok(DefaultConstructorRefV1::Class {
                declaration: declaration
                    .resolve(resolver)
                    .map_err(DefaultConstructorRefResolutionError::ClassDeclaration)?,
                owner_type: owner_type
                    .resolve(resolver)
                    .map_err(DefaultConstructorRefResolutionError::OwnerType)?,
            }),
            Self::Variant {
                declaration,
                owner_type,
            } => Ok(DefaultConstructorRefV1::Variant {
                declaration: resolver
                    .resolve(declaration)
                    .map_err(DefaultConstructorRefResolutionError::VariantDeclaration)?,
                owner_type: owner_type
                    .resolve(resolver)
                    .map_err(DefaultConstructorRefResolutionError::OwnerType)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultConstructorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Struct { .. } => 1,
            Self::Class { .. } => 2,
            Self::Variant { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Struct { declaration, .. } => declaration.encode(encoder)?,
            Self::Class { declaration, .. } => declaration.encode(encoder)?,
            Self::Variant { declaration, .. } => declaration.encode(encoder)?,
        }
        encoder.field(2)?;
        match self {
            Self::Struct { owner_type, .. }
            | Self::Class { owner_type, .. }
            | Self::Variant { owner_type, .. } => owner_type.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultConstructorRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::Struct {
                declaration: decoder.field(1, DecodedPersistentId::decode)?,
                owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            }),
            2 => Ok(Self::Class {
                declaration: decoder.field(1, DecodedDefaultClassConstructorIdV1::decode)?,
                owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            }),
            3 => Ok(Self::Variant {
                declaration: decoder.field(1, DecodedPersistentId::decode)?,
                owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub trait DefaultClassConstructorIdResolver<E>:
    PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

impl<R, E> DefaultClassConstructorIdResolver<E> for R where
    R: PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

pub trait DefaultConstructorReferenceResolver<E>:
    DefaultClassConstructorIdResolver<E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + SignatureTypeReferenceResolver<E>
{
}

impl<R, E> DefaultConstructorReferenceResolver<E> for R where
    R: DefaultClassConstructorIdResolver<E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + SignatureTypeReferenceResolver<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultConstructorRefResolutionError<E> {
    StructDeclaration(E),
    ClassDeclaration(E),
    VariantDeclaration(E),
    OwnerType(E),
}

impl<E: fmt::Display> fmt::Display for DefaultConstructorRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructDeclaration(error) => {
                write!(formatter, "invalid default struct constructor: {error}")
            }
            Self::ClassDeclaration(error) => {
                write!(formatter, "invalid default class constructor: {error}")
            }
            Self::VariantDeclaration(error) => {
                write!(formatter, "invalid default variant constructor: {error}")
            }
            Self::OwnerType(error) => {
                write!(formatter, "invalid default constructor owner: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultConstructorRefResolutionError<E> {}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
