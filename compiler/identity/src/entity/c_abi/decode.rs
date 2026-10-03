use std::fmt;

use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CDataPointee, CPointerStorage, CanonicalCAbiError, CanonicalCStorageType, IntegerBitWidth,
    Signedness, TargetCallingConvention,
};
use crate::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, DecodedPersistentId,
    PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
};

mod layout;
mod signature;

pub use layout::{
    DecodedCLayoutOverride, DecodedCanonicalCAbiLayout, DecodedCanonicalCAbiLayoutField,
    DecodedCanonicalCAbiLayoutFingerprintRecord,
};
pub use signature::{
    DecodedCanonicalCAbiFunctionSignature, DecodedCanonicalCAbiParameter,
    DecodedCanonicalCAbiReturn, DecodedCanonicalCAbiSignatureFingerprintRecord,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCDataPointee {
    OpaqueUnit,
    ExactObject(DecodedPersistentId<PersistentExactTypeId>),
}

impl DecodedCDataPointee {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CDataPointee, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::OpaqueUnit => Ok(CDataPointee::OpaqueUnit),
            Self::ExactObject(id) => resolver.resolve(id).map(CDataPointee::ExactObject),
        }
    }
}

impl WireEncode for DecodedCDataPointee {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OpaqueUnit => encode_empty_sum(encoder, 1),
            Self::ExactObject(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedCDataPointee {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::OpaqueUnit)
            }
            2 => decode_id_variant(decoder, fields, Self::ExactObject),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCPointerStorage {
    Direct,
    NullableWrapper(DecodedPersistentId<PersistentExactTypeId>),
}

impl DecodedCPointerStorage {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CPointerStorage, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Direct => Ok(CPointerStorage::Direct),
            Self::NullableWrapper(id) => resolver.resolve(id).map(CPointerStorage::NullableWrapper),
        }
    }
}

impl WireEncode for DecodedCPointerStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct => encode_empty_sum(encoder, 1),
            Self::NullableWrapper(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedCPointerStorage {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Direct)
            }
            2 => decode_id_variant(decoder, fields, Self::NullableWrapper),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCanonicalCStorageType {
    Integer {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
        signedness: Signedness,
        bit_width: IntegerBitWidth,
    },
    Boolean {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
    },
    DataPointer {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
        pointee: DecodedCDataPointee,
        storage: DecodedCPointerStorage,
    },
    CodePointer {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
        storage: DecodedCPointerStorage,
    },
    Struct {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
        layout: DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    },
}

impl DecodedCanonicalCStorageType {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CanonicalCStorageType, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        match self {
            Self::Integer {
                exact_type,
                signedness,
                bit_width,
            } => Ok(CanonicalCStorageType::Integer {
                exact_type: resolver.resolve(exact_type)?,
                signedness,
                bit_width,
            }),
            Self::Boolean { exact_type } => Ok(CanonicalCStorageType::Boolean {
                exact_type: resolver.resolve(exact_type)?,
            }),
            Self::DataPointer {
                exact_type,
                pointee,
                storage,
            } => Ok(CanonicalCStorageType::DataPointer {
                exact_type: resolver.resolve(exact_type)?,
                pointee: pointee.resolve(resolver)?,
                storage: storage.resolve(resolver)?,
            }),
            Self::CodePointer {
                exact_type,
                storage,
            } => Ok(CanonicalCStorageType::CodePointer {
                exact_type: resolver.resolve(exact_type)?,
                storage: storage.resolve(resolver)?,
            }),
            Self::Struct { exact_type, layout } => Ok(CanonicalCStorageType::Struct {
                exact_type: resolver.resolve(exact_type)?,
                layout: resolver.resolve(layout)?,
            }),
        }
    }
}

impl WireEncode for DecodedCanonicalCStorageType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Integer {
                exact_type,
                signedness,
                bit_width,
            } => encode_three_value_sum(encoder, 1, exact_type, signedness, bit_width),
            Self::Boolean { exact_type } => encode_value_sum(encoder, 2, exact_type),
            Self::DataPointer {
                exact_type,
                pointee,
                storage,
            } => encode_three_value_sum(encoder, 3, exact_type, pointee, storage),
            Self::CodePointer {
                exact_type,
                storage,
            } => encode_two_value_sum(encoder, 4, exact_type, storage),
            Self::Struct { exact_type, layout } => {
                encode_two_value_sum(encoder, 5, exact_type, layout)
            }
        }
    }
}

impl WireDecode for DecodedCanonicalCStorageType {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Integer {
                    exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                    signedness: decoder.field(2, Signedness::decode)?,
                    bit_width: decoder.field(3, IntegerBitWidth::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::Boolean {
                    exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::DataPointer {
                    exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                    pointee: decoder.field(2, DecodedCDataPointee::decode)?,
                    storage: decoder.field(3, DecodedCPointerStorage::decode)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::CodePointer {
                    exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                    storage: decoder.field(2, DecodedCPointerStorage::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Struct {
                    exact_type: decoder.field(1, DecodedPersistentId::decode)?,
                    layout: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for Signedness {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Signed),
            2 => Ok(Self::Unsigned),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for IntegerBitWidth {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            8 => Ok(Self::Bits8),
            16 => Ok(Self::Bits16),
            32 => Ok(Self::Bits32),
            64 => Ok(Self::Bits64),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for TargetCallingConvention {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Cdecl),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalCAbiResolutionError<E> {
    Reference(E),
    Shape(CanonicalCAbiError),
    Hash(HashError),
    SignatureFingerprint(PersistentIdMismatch<CanonicalCAbiSignatureFingerprint>),
    LayoutFingerprint(PersistentIdMismatch<CanonicalCAbiLayoutFingerprint>),
}

impl<E: fmt::Display> fmt::Display for CanonicalCAbiResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::SignatureFingerprint(error) => error.fmt(formatter),
            Self::LayoutFingerprint(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CanonicalCAbiResolutionError<E> {}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
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

fn encode_three_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
    third: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)?;
    encoder.field(3)?;
    third.encode(encoder)
}

#[cfg(test)]
mod tests;
