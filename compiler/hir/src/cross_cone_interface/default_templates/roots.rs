use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DecodedPersistentId, PersistentConstructorId, PersistentEnumVariantId,
    PersistentFunctionId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentPropertyAccessorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod semantics;

pub use semantics::{
    DefaultTemplateProviderShapeBuildError, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, DefaultTemplateRootSemanticValidationError,
};

/// Persistent source root for one exported default template.
///
/// Generated callables and property accessors cannot own source parameters,
/// so neither can be represented by this closed sum.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PersistentLexicalRootV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    EnumVariantConstructor(PersistentEnumVariantId),
}

impl PersistentLexicalRootV1 {
    pub const fn declaration(self) -> CallableTemplateOrigin {
        match self {
            Self::Function(declaration) => CallableTemplateOrigin::Function(declaration),
            Self::GenericFunction(declaration) => {
                CallableTemplateOrigin::GenericFunction(declaration)
            }
            Self::Constructor(declaration) => CallableTemplateOrigin::Constructor(declaration),
            Self::EnumVariantConstructor(declaration) => {
                CallableTemplateOrigin::VariantConstructor(declaration)
            }
        }
    }
}

impl TryFrom<CallableTemplateOrigin> for PersistentLexicalRootV1 {
    type Error = PersistentLexicalRootBuildError;

    fn try_from(value: CallableTemplateOrigin) -> Result<Self, Self::Error> {
        match value {
            CallableTemplateOrigin::Function(declaration) => Ok(Self::Function(declaration)),
            CallableTemplateOrigin::GenericFunction(declaration) => {
                Ok(Self::GenericFunction(declaration))
            }
            CallableTemplateOrigin::Constructor(declaration) => Ok(Self::Constructor(declaration)),
            CallableTemplateOrigin::VariantConstructor(declaration) => {
                Ok(Self::EnumVariantConstructor(declaration))
            }
            CallableTemplateOrigin::Accessor(accessor) => {
                Err(PersistentLexicalRootBuildError::PropertyAccessor(accessor))
            }
        }
    }
}

impl WireEncode for PersistentLexicalRootV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::Constructor(_) => 3,
            Self::EnumVariantConstructor(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(declaration) => declaration.encode(encoder),
            Self::GenericFunction(declaration) => declaration.encode(encoder),
            Self::Constructor(declaration) => declaration.encode(encoder),
            Self::EnumVariantConstructor(declaration) => declaration.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedPersistentLexicalRootV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    EnumVariantConstructor(DecodedPersistentId<PersistentEnumVariantId>),
}

impl DecodedPersistentLexicalRootV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PersistentLexicalRootV1, E>
    where
        R: PersistentLexicalRootResolver<E>,
    {
        match self {
            Self::Function(declaration) => resolver
                .resolve(declaration)
                .map(PersistentLexicalRootV1::Function),
            Self::GenericFunction(declaration) => resolver
                .resolve(declaration)
                .map(PersistentLexicalRootV1::GenericFunction),
            Self::Constructor(declaration) => resolver
                .resolve(declaration)
                .map(PersistentLexicalRootV1::Constructor),
            Self::EnumVariantConstructor(declaration) => resolver
                .resolve(declaration)
                .map(PersistentLexicalRootV1::EnumVariantConstructor),
        }
    }
}

impl WireEncode for DecodedPersistentLexicalRootV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::Constructor(_) => 3,
            Self::EnumVariantConstructor(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(declaration) => declaration.encode(encoder),
            Self::GenericFunction(declaration) => declaration.encode(encoder),
            Self::Constructor(declaration) => declaration.encode(encoder),
            Self::EnumVariantConstructor(declaration) => declaration.encode(encoder),
        }
    }
}

impl WireDecode for DecodedPersistentLexicalRootV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
            ));
        }
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariantConstructor),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub trait PersistentLexicalRootResolver<E>:
    PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

impl<R, E> PersistentLexicalRootResolver<E> for R where
    R: PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentLexicalRootBuildError {
    PropertyAccessor(PersistentPropertyAccessorId),
}

impl fmt::Display for PersistentLexicalRootBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PropertyAccessor(accessor) => write!(
                formatter,
                "property accessor {accessor} cannot be a default-template lexical root"
            ),
        }
    }
}

impl std::error::Error for PersistentLexicalRootBuildError {}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
