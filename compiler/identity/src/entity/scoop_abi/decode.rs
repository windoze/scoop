use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError,
    ScoopAbiReturn, ScoopAbiValueShape,
};
use crate::{
    AbiCoercion, DecodedExactCallableSignature, DecodedPersistentId,
    ExactCallableSignatureResolutionError, GcEffect, PersistentExactTypeId, PersistentIdResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedCanonicalScoopStorage {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    byte_size: u64,
    alignment: NonZeroU64,
    shape: ScoopAbiValueShape,
}

impl DecodedCanonicalScoopStorage {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CanonicalScoopStorage, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        Ok(CanonicalScoopStorage::new(
            resolver.resolve(self.exact_type)?,
            self.byte_size,
            self.alignment,
            self.shape,
        ))
    }
}

impl WireEncode for DecodedCanonicalScoopStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_size)?;
        encoder.field(3)?;
        encoder.unsigned(self.alignment.get())?;
        encoder.field(4)?;
        self.shape.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalScoopStorage {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            byte_size: decoder.field(2, Decoder::unsigned)?,
            alignment: decoder.field(3, decode_non_zero_u64)?,
            shape: decoder.field(4, ScoopAbiValueShape::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedScoopAbiArgument {
    ElidedZst(DecodedCanonicalScoopStorage),
    Direct(DecodedCanonicalScoopStorage),
    Indirect(DecodedCanonicalScoopStorage),
    DirectParts(DecodedCanonicalScoopStorage, AbiCoercion),
}

impl DecodedScoopAbiArgument {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ScoopAbiArgument, ScoopAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::ElidedZst(storage) => ScoopAbiArgument::elided_zst(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            ),
            Self::Direct(storage) => ScoopAbiArgument::direct(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            ),
            Self::Indirect(storage) => ScoopAbiArgument::indirect(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            ),
            Self::DirectParts(storage, coercion) => ScoopAbiArgument::direct_parts(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
                coercion,
            ),
        }
        .map_err(ScoopAbiResolutionError::Shape)
    }
}

impl WireEncode for DecodedScoopAbiArgument {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ElidedZst(storage) => encode_value_sum(encoder, 1, storage),
            Self::Direct(storage) => encode_value_sum(encoder, 2, storage),
            Self::Indirect(storage) => encode_value_sum(encoder, 3, storage),
            Self::DirectParts(storage, coercion) => encode_parts(encoder, 4, storage, coercion),
        }
    }
}

impl WireDecode for DecodedScoopAbiArgument {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, if tag == 4 { 3 } else { 2 })?;
        match tag {
            1 => decode_storage_variant(decoder, Self::ElidedZst),
            2 => decode_storage_variant(decoder, Self::Direct),
            3 => decode_storage_variant(decoder, Self::Indirect),
            4 => decode_parts(decoder, Self::DirectParts),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedScoopAbiReturn {
    UnitVoid,
    ElidedZst(DecodedCanonicalScoopStorage),
    Direct(DecodedCanonicalScoopStorage),
    Indirect(DecodedCanonicalScoopStorage),
    DirectParts(DecodedCanonicalScoopStorage, AbiCoercion),
}

impl DecodedScoopAbiReturn {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ScoopAbiReturn, ScoopAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::UnitVoid => Ok(ScoopAbiReturn::unit_void()),
            Self::ElidedZst(storage) => ScoopAbiReturn::elided_zst(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            )
            .map_err(ScoopAbiResolutionError::Shape),
            Self::Direct(storage) => ScoopAbiReturn::direct(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            )
            .map_err(ScoopAbiResolutionError::Shape),
            Self::Indirect(storage) => ScoopAbiReturn::indirect(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
            )
            .map_err(ScoopAbiResolutionError::Shape),
            Self::DirectParts(storage, coercion) => ScoopAbiReturn::direct_parts(
                storage
                    .resolve(resolver)
                    .map_err(ScoopAbiResolutionError::Reference)?,
                coercion,
            )
            .map_err(ScoopAbiResolutionError::Shape),
        }
    }
}

impl WireEncode for DecodedScoopAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::UnitVoid => encode_empty_sum(encoder, 1),
            Self::ElidedZst(storage) => encode_value_sum(encoder, 2, storage),
            Self::Direct(storage) => encode_value_sum(encoder, 3, storage),
            Self::Indirect(storage) => encode_value_sum(encoder, 4, storage),
            Self::DirectParts(storage, coercion) => encode_parts(encoder, 5, storage, coercion),
        }
    }
}

impl WireDecode for DecodedScoopAbiReturn {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::UnitVoid)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decode_storage_variant(decoder, Self::ElidedZst)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decode_storage_variant(decoder, Self::Direct)
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decode_storage_variant(decoder, Self::Indirect)
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                decode_parts(decoder, Self::DirectParts)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalScoopAbiFunctionSignature {
    signature: DecodedExactCallableSignature,
    arguments: Vec<DecodedScoopAbiArgument>,
    result: DecodedScoopAbiReturn,
    gc_effect: GcEffect,
}

impl DecodedCanonicalScoopAbiFunctionSignature {
    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn signature_parameter_count(&self) -> usize {
        self.signature.parameter_count()
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalScoopAbiFunctionSignature, ScoopAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        let signature = self
            .signature
            .resolve(resolver)
            .map_err(ScoopAbiResolutionError::Signature)?;
        let arguments = self
            .arguments
            .into_iter()
            .map(|argument| argument.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.result.resolve(resolver)?;
        CanonicalScoopAbiFunctionSignature::new(signature, arguments, result, self.gc_effect)
            .map_err(ScoopAbiResolutionError::Shape)
    }
}

impl WireEncode for DecodedCanonicalScoopAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.signature.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)?;
        encoder.field(4)?;
        self.gc_effect.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalScoopAbiFunctionSignature {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            signature: decoder.field(1, DecodedExactCallableSignature::decode)?,
            arguments: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedScoopAbiArgument::decode(decoder))
            })?,
            result: decoder.field(3, DecodedScoopAbiReturn::decode)?,
            gc_effect: decoder.field(4, GcEffect::decode)?,
        })
    }
}

impl WireDecode for ScoopAbiValueShape {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Scalar),
            2 => Ok(Self::Aggregate),
            3 => Ok(Self::Interface),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopAbiResolutionError<E> {
    Reference(E),
    Signature(ExactCallableSignatureResolutionError<E>),
    Shape(ScoopAbiError),
}

impl<E: fmt::Display> fmt::Display for ScoopAbiResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Signature(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScoopAbiResolutionError<E> {}

fn decode_non_zero_u64(decoder: &mut Decoder<'_>) -> Result<NonZeroU64, WireError> {
    NonZeroU64::new(decoder.unsigned()?).ok_or_else(|| {
        WireError::new(
            WireErrorKind::IntegerOutOfRange,
            decoder.path().clone(),
            Some(decoder.position()),
        )
    })
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_storage_variant<T>(
    decoder: &mut Decoder<'_>,
    build: impl FnOnce(DecodedCanonicalScoopStorage) -> T,
) -> Result<T, WireError> {
    decoder
        .field(1, DecodedCanonicalScoopStorage::decode)
        .map(build)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
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

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
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

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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

#[cfg(test)]
mod tests;

fn encode_parts(
    e: &mut Encoder,
    tag: u64,
    storage: &DecodedCanonicalScoopStorage,
    coercion: &AbiCoercion,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(3)?;
    encode_tag(e, tag)?;
    e.field(1)?;
    storage.encode(e)?;
    e.field(2)?;
    coercion.encode(e)
}
fn decode_parts<T>(
    d: &mut Decoder<'_>,
    wrap: impl FnOnce(DecodedCanonicalScoopStorage, AbiCoercion) -> T,
) -> Result<T, WireError> {
    Ok(wrap(
        d.field(1, DecodedCanonicalScoopStorage::decode)?,
        d.field(2, AbiCoercion::decode)?,
    ))
}
