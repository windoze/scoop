use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallingConvention, DuplicateSignatureKey, Effect, NonEmptyVec, OptionalSignatureType,
    SignatureConditionKey, SignatureTypeKey, encode_sequence, encode_single_payload, encode_tag,
};
use crate::{DecodedPersistentId, PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedSignatureTypeKey {
    Nominal(DecodedPersistentId<PersistentTypeId>),
    NominalApplication {
        origin: DecodedPersistentId<PersistentGenericTypeId>,
        arguments: NonEmptyVec<DecodedSignatureTypeKey>,
    },
    Tuple(NonEmptyVec<DecodedSignatureTypeKey>),
    Function {
        effect: Effect,
        parameters: Vec<DecodedSignatureTypeKey>,
        result: Box<DecodedSignatureTypeKey>,
        receiver: Option<Box<DecodedSignatureTypeKey>>,
    },
    RawPointer(Box<DecodedSignatureTypeKey>),
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: Vec<DecodedSignatureTypeKey>,
        result: Box<DecodedSignatureTypeKey>,
    },
    Binder {
        depth: u32,
        index: u32,
    },
    Companion(Box<DecodedSignatureTypeKey>),
}

impl DecodedSignatureTypeKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SignatureTypeKey, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Nominal(id) => resolver.resolve(id).map(SignatureTypeKey::Nominal),
            Self::NominalApplication { origin, arguments } => {
                let origin = resolver.resolve(origin)?;
                let arguments = resolve_non_empty(arguments, resolver)?;
                Ok(SignatureTypeKey::NominalApplication { origin, arguments })
            }
            Self::Tuple(elements) => {
                resolve_non_empty(elements, resolver).map(SignatureTypeKey::Tuple)
            }
            Self::Function {
                receiver,
                effect,
                parameters,
                result,
            } => Ok(SignatureTypeKey::Function {
                receiver: receiver
                    .map(|ty| ty.resolve(resolver).map(Box::new))
                    .transpose()?,
                effect,
                parameters: resolve_sequence(parameters, resolver)?,
                result: Box::new(result.resolve(resolver)?),
            }),
            Self::RawPointer(pointee) => Ok(SignatureTypeKey::RawPointer(Box::new(
                pointee.resolve(resolver)?,
            ))),
            Self::Companion(host) => Ok(SignatureTypeKey::Companion(Box::new(
                host.resolve(resolver)?,
            ))),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => Ok(SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters: resolve_sequence(parameters, resolver)?,
                result: Box::new(result.resolve(resolver)?),
            }),
            Self::Binder { depth, index } => Ok(SignatureTypeKey::Binder { depth, index }),
        }
    }
}

impl WireEncode for DecodedSignatureTypeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(id) => encode_single_payload(encoder, 1, id),
            Self::NominalApplication { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, arguments.as_slice())
            }
            Self::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements.as_slice())
            }
            Self::Function {
                receiver,
                effect,
                parameters,
                result,
            } => {
                encoder.map(4 + u64::from(receiver.is_some()))?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                effect.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)?;
                if let Some(receiver) = receiver {
                    encoder.field(4)?;
                    receiver.encode(encoder)?;
                }
                Ok(())
            }
            Self::RawPointer(pointee) => encode_single_payload(encoder, 5, pointee.as_ref()),
            Self::Companion(host) => encode_single_payload(encoder, 8, host.as_ref()),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                calling_convention.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::Binder { depth, index } => {
                encoder.map(3)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*depth))?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*index))
            }
        }
    }
}

impl WireDecode for DecodedSignatureTypeKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Nominal)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::NominalApplication {
                    origin: decoder.field(1, DecodedPersistentId::decode)?,
                    arguments: decoder.field(2, decode_non_empty_signatures)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, decode_non_empty_signatures)
                    .map(Self::Tuple)
            }
            4 => {
                if fields != 5 {
                    expect_sum_length(decoder, fields, 4)?;
                }
                Ok(Self::Function {
                    effect: decoder.field(1, decode_effect)?,
                    parameters: decoder.field(2, decode_signatures)?,
                    result: decoder.field(3, |decoder| {
                        DecodedSignatureTypeKey::decode(decoder).map(Box::new)
                    })?,
                    receiver: if fields == 5 {
                        Some(decoder.field(4, |decoder| Self::decode(decoder).map(Box::new))?)
                    } else {
                        None
                    },
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(Box::new)
                    .map(Self::RawPointer)
            }
            6 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::NativeFunctionPointer {
                    calling_convention: decoder.field(1, decode_calling_convention)?,
                    parameters: decoder.field(2, decode_signatures)?,
                    result: decoder.field(3, |decoder| {
                        DecodedSignatureTypeKey::decode(decoder).map(Box::new)
                    })?,
                })
            }
            7 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Binder {
                    depth: decoder.field(1, Decoder::u32)?,
                    index: decoder.field(2, Decoder::u32)?,
                })
            }
            8 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(Box::new)
                    .map(Self::Companion)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedOptionalSignatureType {
    Absent,
    Present(Box<DecodedSignatureTypeKey>),
}

impl DecodedOptionalSignatureType {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<OptionalSignatureType, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Absent => Ok(OptionalSignatureType::Absent),
            Self::Present(value) => value
                .resolve(resolver)
                .map(Box::new)
                .map(OptionalSignatureType::Present),
        }
    }
}

impl WireEncode for DecodedOptionalSignatureType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Present(value) => encode_single_payload(encoder, 2, value.as_ref()),
        }
    }
}

impl WireDecode for DecodedOptionalSignatureType {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(Box::new)
                    .map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedDuplicateSignatureKey {
    Nominal {
        type_parameter_count: u32,
    },
    Function {
        type_parameter_count: u32,
        receiver: DecodedOptionalSignatureType,
        parameters: Vec<DecodedSignatureTypeKey>,
        conditions: Vec<SignatureConditionKey<DecodedSignatureTypeKey>>,
    },
    Constructor {
        parameters: Vec<DecodedSignatureTypeKey>,
    },
    Property {
        type_parameter_count: u32,
        receiver: DecodedOptionalSignatureType,
    },
    TypeAlias,
}

impl DecodedDuplicateSignatureKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DuplicateSignatureKey, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Nominal {
                type_parameter_count,
            } => Ok(DuplicateSignatureKey::Nominal {
                type_parameter_count,
            }),
            Self::Function {
                type_parameter_count,
                receiver,
                parameters,
                conditions,
            } => Ok(DuplicateSignatureKey::Function {
                type_parameter_count,
                receiver: receiver.resolve(resolver)?,
                parameters: resolve_sequence(parameters, resolver)?,
                conditions: conditions
                    .into_iter()
                    .map(|condition| condition.resolve(resolver))
                    .collect::<Result<_, _>>()?,
            }),
            Self::Constructor { parameters } => Ok(DuplicateSignatureKey::Constructor {
                parameters: resolve_sequence(parameters, resolver)?,
            }),
            Self::Property {
                type_parameter_count,
                receiver,
            } => Ok(DuplicateSignatureKey::Property {
                type_parameter_count,
                receiver: receiver.resolve(resolver)?,
            }),
            Self::TypeAlias => Ok(DuplicateSignatureKey::TypeAlias),
        }
    }
}

impl WireEncode for DecodedDuplicateSignatureKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal {
                type_parameter_count,
            } => {
                encoder.map(2)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))
            }
            Self::Function {
                type_parameter_count,
                receiver,
                parameters,
                conditions,
            } => {
                encoder.map(4 + u64::from(!conditions.is_empty()))?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))?;
                encoder.field(2)?;
                receiver.encode(encoder)?;
                encoder.field(3)?;
                encode_sequence(encoder, parameters)?;
                if !conditions.is_empty() {
                    encoder.field(4)?;
                    encode_sequence(encoder, conditions)?;
                }
                Ok(())
            }
            Self::Constructor { parameters } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, parameters)
            }
            Self::Property {
                type_parameter_count,
                receiver,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))?;
                encoder.field(2)?;
                receiver.encode(encoder)
            }
            Self::TypeAlias => {
                encoder.map(1)?;
                encode_tag(encoder, 5)
            }
        }
    }
}

impl WireDecode for DecodedDuplicateSignatureKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::Nominal {
                    type_parameter_count: decoder.field(1, Decoder::u32)?,
                })
            }
            2 => {
                if fields != 4 && fields != 5 {
                    return Err(wire_error(
                        decoder,
                        WireErrorKind::InvalidLength {
                            expected: 4,
                            actual: fields,
                        },
                    ));
                }
                Ok(Self::Function {
                    type_parameter_count: decoder.field(1, Decoder::u32)?,
                    receiver: decoder.field(2, DecodedOptionalSignatureType::decode)?,
                    parameters: decoder.field(3, decode_signatures)?,
                    conditions: if fields == 5 {
                        decoder.field(4, |decoder| {
                            decoder
                                .decode_array(|decoder, _| SignatureConditionKey::decode(decoder))
                        })?
                    } else {
                        Vec::new()
                    },
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, decode_signatures)
                    .map(|parameters| Self::Constructor { parameters })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Property {
                    type_parameter_count: decoder.field(1, Decoder::u32)?,
                    receiver: decoder.field(2, DecodedOptionalSignatureType::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::TypeAlias)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn resolve_non_empty<R, E>(
    values: NonEmptyVec<DecodedSignatureTypeKey>,
    resolver: &mut R,
) -> Result<NonEmptyVec<SignatureTypeKey>, E>
where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
{
    let values = resolve_sequence(values.0, resolver)?;
    Ok(NonEmptyVec(values))
}

fn resolve_sequence<R, E>(
    values: Vec<DecodedSignatureTypeKey>,
    resolver: &mut R,
) -> Result<Vec<SignatureTypeKey>, E>
where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
{
    values
        .into_iter()
        .map(|value| value.resolve(resolver))
        .collect()
}

fn decode_signatures(decoder: &mut Decoder<'_>) -> Result<Vec<DecodedSignatureTypeKey>, WireError> {
    decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
}

fn decode_non_empty_signatures(
    decoder: &mut Decoder<'_>,
) -> Result<NonEmptyVec<DecodedSignatureTypeKey>, WireError> {
    let values = decode_signatures(decoder)?;
    NonEmptyVec::new(values).map_err(|_| {
        wire_error(
            decoder,
            WireErrorKind::InvalidLength {
                expected: 1,
                actual: 0,
            },
        )
    })
}

fn decode_effect(decoder: &mut Decoder<'_>) -> Result<Effect, WireError> {
    match decoder.unsigned()? {
        1 => Ok(Effect::Ordinary),
        2 => Ok(Effect::Suspend),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn decode_calling_convention(decoder: &mut Decoder<'_>) -> Result<CallingConvention, WireError> {
    match decoder.unsigned()? {
        1 => Ok(CallingConvention::C),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
