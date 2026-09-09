use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::SourceContextKey;
use crate::{
    ConeIdentity, DecodedCallableOwner, DecodedNominalDeclarationOwner, DecodedPropertyOwner,
    DecodedSourceIdentity, PersistentCallableApplicationId, PersistentConstructorId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentTypeId, SourceIdentityResolutionError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceContextKey {
    File {
        source: DecodedSourceIdentity,
    },
    Nominal {
        source: DecodedSourceIdentity,
        owner: DecodedNominalDeclarationOwner,
    },
    Callable {
        source: DecodedSourceIdentity,
        owner: DecodedCallableOwner,
    },
    Property {
        source: DecodedSourceIdentity,
        owner: DecodedPropertyOwner,
    },
    Initialization {
        source: DecodedSourceIdentity,
        unit: crate::DecodedPersistentId<PersistentInitializationUnitId>,
    },
}

impl DecodedSourceContextKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceContextKey, SourceContextResolutionError<E>>
    where
        R: SourceContextResolver<E>,
    {
        match self {
            Self::File { source } => {
                resolve_source(source, resolver).map(|source| SourceContextKey::File { source })
            }
            Self::Nominal { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Nominal { source, owner })
            }
            Self::Callable { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Callable { source, owner })
            }
            Self::Property { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Property { source, owner })
            }
            Self::Initialization { source, unit } => {
                let source = resolve_source(source, resolver)?;
                let unit = resolver
                    .resolve(unit)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Initialization { source, unit })
            }
        }
    }
}

impl WireEncode for DecodedSourceContextKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::File { source } => encode_context(encoder, 1, source, None),
            Self::Nominal { source, owner } => encode_context(encoder, 2, source, Some(owner)),
            Self::Callable { source, owner } => encode_context(encoder, 3, source, Some(owner)),
            Self::Property { source, owner } => encode_context(encoder, 4, source, Some(owner)),
            Self::Initialization { source, unit } => encode_context(encoder, 5, source, Some(unit)),
        }
    }
}

impl WireDecode for DecodedSourceContextKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceIdentity::decode)
                    .map(|source| Self::File { source })
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Nominal {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedNominalDeclarationOwner::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Callable {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedCallableOwner::decode)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Property {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedPropertyOwner::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Initialization {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    unit: decoder.field(2, crate::DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

pub trait SourceContextResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

impl<R, E> SourceContextResolver<E> for R where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceContextResolutionError<E> {
    Source(SourceIdentityResolutionError<E>),
    Reference(E),
}

impl<E: fmt::Display> fmt::Display for SourceContextResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Reference(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceContextResolutionError<E> {}

fn resolve_source<R, E>(
    source: DecodedSourceIdentity,
    resolver: &mut R,
) -> Result<crate::SourceIdentity, SourceContextResolutionError<E>>
where
    R: PersistentIdResolver<ConeIdentity, Error = E>,
{
    source
        .resolve(resolver)
        .map_err(SourceContextResolutionError::Source)
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

fn encode_context(
    encoder: &mut Encoder,
    tag: u64,
    source: &DecodedSourceIdentity,
    owner: Option<&dyn WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(if owner.is_some() { 3 } else { 2 })?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    if let Some(owner) = owner {
        encoder.field(2)?;
        owner.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
