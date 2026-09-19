mod resolution_nodes;

use std::fmt;

mod resources;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentIdResolver, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::SignatureTypeReferenceResolver;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultEnumVariantRefV1 {
    declaration: PersistentEnumVariantId,
    owner_type: SignatureTypeKey,
}

impl DefaultEnumVariantRefV1 {
    pub const fn new(declaration: PersistentEnumVariantId, owner_type: SignatureTypeKey) -> Self {
        Self {
            declaration,
            owner_type,
        }
    }

    pub const fn declaration(&self) -> PersistentEnumVariantId {
        self.declaration
    }

    pub const fn owner_type(&self) -> &SignatureTypeKey {
        &self.owner_type
    }
}

impl WireEncode for DefaultEnumVariantRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultEnumVariantRefV1 {
    declaration: DecodedPersistentId<PersistentEnumVariantId>,
    owner_type: DecodedSignatureTypeKey,
}

impl DecodedDefaultEnumVariantRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultEnumVariantRefV1, DefaultEnumVariantRefResolutionError<E>>
    where
        R: DefaultFieldReferenceResolver<E>,
    {
        Ok(DefaultEnumVariantRefV1 {
            declaration: resolver
                .resolve(self.declaration)
                .map_err(DefaultEnumVariantRefResolutionError::Declaration)?,
            owner_type: self
                .owner_type
                .resolve(resolver)
                .map_err(DefaultEnumVariantRefResolutionError::OwnerType)?,
        })
    }
}

impl WireEncode for DecodedDefaultEnumVariantRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner_type.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultEnumVariantRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultEnumVariantFieldRefV1 {
    declaration: PersistentEnumVariantFieldId,
    owner_type: SignatureTypeKey,
}

impl DefaultEnumVariantFieldRefV1 {
    pub const fn new(
        declaration: PersistentEnumVariantFieldId,
        owner_type: SignatureTypeKey,
    ) -> Self {
        Self {
            declaration,
            owner_type,
        }
    }

    pub const fn declaration(&self) -> PersistentEnumVariantFieldId {
        self.declaration
    }

    pub const fn owner_type(&self) -> &SignatureTypeKey {
        &self.owner_type
    }
}

impl WireEncode for DefaultEnumVariantFieldRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultEnumVariantFieldRefV1 {
    declaration: DecodedPersistentId<PersistentEnumVariantFieldId>,
    owner_type: DecodedSignatureTypeKey,
}

impl DecodedDefaultEnumVariantFieldRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultEnumVariantFieldRefV1, DefaultEnumVariantFieldRefResolutionError<E>>
    where
        R: DefaultFieldReferenceResolver<E>,
    {
        Ok(DefaultEnumVariantFieldRefV1 {
            declaration: resolver
                .resolve(self.declaration)
                .map_err(DefaultEnumVariantFieldRefResolutionError::Declaration)?,
            owner_type: self
                .owner_type
                .resolve(resolver)
                .map_err(DefaultEnumVariantFieldRefResolutionError::OwnerType)?,
        })
    }
}

impl WireEncode for DecodedDefaultEnumVariantFieldRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner_type.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultEnumVariantFieldRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultFieldRefV1 {
    Struct {
        declaration: PersistentFieldId,
        owner_type: SignatureTypeKey,
    },
    Tuple {
        declaration_index: u32,
    },
    Class {
        declaration: PersistentFieldId,
        owner_type: SignatureTypeKey,
    },
}

impl WireEncode for DefaultFieldRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Struct {
                declaration,
                owner_type,
            } => encode_declared_field(encoder, 1, declaration, owner_type),
            Self::Tuple { declaration_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*declaration_index))
            }
            Self::Class {
                declaration,
                owner_type,
            } => encode_declared_field(encoder, 3, declaration, owner_type),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultFieldRefV1 {
    Struct {
        declaration: DecodedPersistentId<PersistentFieldId>,
        owner_type: DecodedSignatureTypeKey,
    },
    Tuple {
        declaration_index: u32,
    },
    Class {
        declaration: DecodedPersistentId<PersistentFieldId>,
        owner_type: DecodedSignatureTypeKey,
    },
}

impl DecodedDefaultFieldRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultFieldRefV1, DefaultFieldRefResolutionError<E>>
    where
        R: DefaultFieldReferenceResolver<E>,
    {
        match self {
            Self::Struct {
                declaration,
                owner_type,
            } => Ok(DefaultFieldRefV1::Struct {
                declaration: resolver
                    .resolve(declaration)
                    .map_err(DefaultFieldRefResolutionError::StructDeclaration)?,
                owner_type: owner_type
                    .resolve(resolver)
                    .map_err(DefaultFieldRefResolutionError::StructOwnerType)?,
            }),
            Self::Tuple { declaration_index } => Ok(DefaultFieldRefV1::Tuple { declaration_index }),
            Self::Class {
                declaration,
                owner_type,
            } => Ok(DefaultFieldRefV1::Class {
                declaration: resolver
                    .resolve(declaration)
                    .map_err(DefaultFieldRefResolutionError::ClassDeclaration)?,
                owner_type: owner_type
                    .resolve(resolver)
                    .map_err(DefaultFieldRefResolutionError::ClassOwnerType)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultFieldRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Struct {
                declaration,
                owner_type,
            } => encode_declared_field(encoder, 1, declaration, owner_type),
            Self::Tuple { declaration_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*declaration_index))
            }
            Self::Class {
                declaration,
                owner_type,
            } => encode_declared_field(encoder, 3, declaration, owner_type),
        }
    }
}

impl WireDecode for DecodedDefaultFieldRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Struct {
                    declaration: decoder.field(1, DecodedPersistentId::decode)?,
                    owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::u32)
                    .map(|declaration_index| Self::Tuple { declaration_index })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Class {
                    declaration: decoder.field(1, DecodedPersistentId::decode)?,
                    owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub trait DefaultFieldReferenceResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
    + PersistentIdResolver<PersistentFieldId, Error = E>
{
}

impl<R, E> DefaultFieldReferenceResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
        + PersistentIdResolver<PersistentFieldId, Error = E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultEnumVariantRefResolutionError<E> {
    Declaration(E),
    OwnerType(E),
}

impl<E: fmt::Display> fmt::Display for DefaultEnumVariantRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid default enum variant: {error}"),
            Self::OwnerType(error) => {
                write!(formatter, "invalid default enum variant owner: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultEnumVariantRefResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultEnumVariantFieldRefResolutionError<E> {
    Declaration(E),
    OwnerType(E),
}

impl<E: fmt::Display> fmt::Display for DefaultEnumVariantFieldRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => {
                write!(formatter, "invalid default enum variant field: {error}")
            }
            Self::OwnerType(error) => {
                write!(
                    formatter,
                    "invalid default enum variant field owner: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultEnumVariantFieldRefResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultFieldRefResolutionError<E> {
    StructDeclaration(E),
    StructOwnerType(E),
    ClassDeclaration(E),
    ClassOwnerType(E),
}

impl<E: fmt::Display> fmt::Display for DefaultFieldRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructDeclaration(error) => {
                write!(formatter, "invalid default struct field: {error}")
            }
            Self::StructOwnerType(error) => {
                write!(formatter, "invalid default struct field owner: {error}")
            }
            Self::ClassDeclaration(error) => {
                write!(formatter, "invalid default class field: {error}")
            }
            Self::ClassOwnerType(error) => {
                write!(formatter, "invalid default class field owner: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultFieldRefResolutionError<E> {}

fn encode_declared_field(
    encoder: &mut Encoder,
    tag: u64,
    declaration: &impl WireEncode,
    owner_type: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    declaration.encode(encoder)?;
    encoder.field(2)?;
    owner_type.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

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
