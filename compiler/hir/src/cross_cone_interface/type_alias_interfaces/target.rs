use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentIdResolver, PersistentTypeAliasId,
    SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::SignatureTypeReferenceResolver;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeAliasTargetV1 {
    Signature(SignatureTypeKey),
    Alias(PersistentTypeAliasId),
}

impl WireEncode for TypeAliasTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Signature(_) => 1,
            Self::Alias(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Signature(target) => target.encode(encoder),
            Self::Alias(alias) => alias.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedTypeAliasTargetV1 {
    Signature(DecodedSignatureTypeKey),
    Alias(DecodedPersistentId<PersistentTypeAliasId>),
}

impl DecodedTypeAliasTargetV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TypeAliasTargetV1, TypeAliasTargetResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<PersistentTypeAliasId, Error = E>,
    {
        match self {
            Self::Signature(target) => target
                .resolve(resolver)
                .map(TypeAliasTargetV1::Signature)
                .map_err(TypeAliasTargetResolutionError::Signature),
            Self::Alias(alias) => resolver
                .resolve(alias)
                .map(TypeAliasTargetV1::Alias)
                .map_err(TypeAliasTargetResolutionError::Alias),
        }
    }
}

impl WireEncode for DecodedTypeAliasTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Signature(_) => 1,
            Self::Alias(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Signature(target) => target.encode(encoder),
            Self::Alias(alias) => alias.encode(encoder),
        }
    }
}

impl WireDecode for DecodedTypeAliasTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            1 => decoder
                .field(1, DecodedSignatureTypeKey::decode)
                .map(Self::Signature),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Alias),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Debug)]
pub enum TypeAliasTargetResolutionError<E> {
    Signature(E),
    Alias(E),
}

impl<E: fmt::Display> fmt::Display for TypeAliasTargetResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature(error) => write!(formatter, "invalid type-alias signature: {error}"),
            Self::Alias(error) => write!(formatter, "invalid type-alias target: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TypeAliasTargetResolutionError<E> {}

#[cfg(test)]
mod tests;
