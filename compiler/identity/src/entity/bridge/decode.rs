use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeSemanticTarget,
    GeneratedBridgeUnitKey,
};
use crate::{
    CallbackParameterIndex, CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint,
    ConeIdentity, DecodedPersistentId, GeneratedBridgeUnitId, NativeExternalContractFingerprint,
    PersistentIdResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedGeneratedBridgeUnitKey {
    OutboundFunction(DecodedPersistentId<NativeExternalContractFingerprint>),
    GlobalRead(DecodedPersistentId<NativeExternalContractFingerprint>),
    GlobalWrite(DecodedPersistentId<NativeExternalContractFingerprint>),
    GlobalAddress(DecodedPersistentId<NativeExternalContractFingerprint>),
    CallbackTrampoline {
        signature: DecodedPersistentId<CanonicalCAbiSignatureFingerprint>,
        context_index: CallbackParameterIndex,
    },
}

impl DecodedGeneratedBridgeUnitKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<GeneratedBridgeUnitKey, E>
    where
        R: PersistentIdResolver<NativeExternalContractFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>,
    {
        match self {
            Self::OutboundFunction(contract) => resolver
                .resolve(contract)
                .map(GeneratedBridgeUnitKey::OutboundFunction),
            Self::GlobalRead(contract) => resolver
                .resolve(contract)
                .map(GeneratedBridgeUnitKey::GlobalRead),
            Self::GlobalWrite(contract) => resolver
                .resolve(contract)
                .map(GeneratedBridgeUnitKey::GlobalWrite),
            Self::GlobalAddress(contract) => resolver
                .resolve(contract)
                .map(GeneratedBridgeUnitKey::GlobalAddress),
            Self::CallbackTrampoline {
                signature,
                context_index,
            } => Ok(GeneratedBridgeUnitKey::CallbackTrampoline {
                signature: resolver.resolve(signature)?,
                context_index,
            }),
        }
    }
}

impl WireEncode for DecodedGeneratedBridgeUnitKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OutboundFunction(contract) => encode_value_sum(encoder, 1, contract),
            Self::GlobalRead(contract) => encode_value_sum(encoder, 2, contract),
            Self::GlobalWrite(contract) => encode_value_sum(encoder, 3, contract),
            Self::GlobalAddress(contract) => encode_value_sum(encoder, 4, contract),
            Self::CallbackTrampoline {
                signature,
                context_index,
            } => encode_two_value_sum(encoder, 5, signature, context_index),
        }
    }
}

impl WireDecode for DecodedGeneratedBridgeUnitKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_id_variant(decoder, fields, Self::OutboundFunction),
            2 => decode_id_variant(decoder, fields, Self::GlobalRead),
            3 => decode_id_variant(decoder, fields, Self::GlobalWrite),
            4 => decode_id_variant(decoder, fields, Self::GlobalAddress),
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::CallbackTrampoline {
                    signature: decoder.field(1, DecodedPersistentId::decode)?,
                    context_index: decoder.field(2, CallbackParameterIndex::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedGeneratedBridgeAtomRoleKey {
    PrimaryEntry {
        unit: DecodedPersistentId<GeneratedBridgeUnitId>,
    },
    SignatureDescriptor {
        unit: DecodedPersistentId<GeneratedBridgeUnitId>,
        signature: DecodedPersistentId<CanonicalCAbiSignatureFingerprint>,
    },
    ContextDescriptor {
        unit: DecodedPersistentId<GeneratedBridgeUnitId>,
        context_index: CallbackParameterIndex,
    },
    StaticAssertSupport {
        unit: DecodedPersistentId<GeneratedBridgeUnitId>,
        layout: DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    },
}

impl DecodedGeneratedBridgeAtomRoleKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<GeneratedBridgeAtomRoleKey, E>
    where
        R: PersistentIdResolver<GeneratedBridgeUnitId, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        match self {
            Self::PrimaryEntry { unit } => Ok(GeneratedBridgeAtomRoleKey::PrimaryEntry {
                unit: resolver.resolve(unit)?,
            }),
            Self::SignatureDescriptor { unit, signature } => {
                Ok(GeneratedBridgeAtomRoleKey::SignatureDescriptor {
                    unit: resolver.resolve(unit)?,
                    signature: resolver.resolve(signature)?,
                })
            }
            Self::ContextDescriptor {
                unit,
                context_index,
            } => Ok(GeneratedBridgeAtomRoleKey::ContextDescriptor {
                unit: resolver.resolve(unit)?,
                context_index,
            }),
            Self::StaticAssertSupport { unit, layout } => {
                Ok(GeneratedBridgeAtomRoleKey::StaticAssertSupport {
                    unit: resolver.resolve(unit)?,
                    layout: resolver.resolve(layout)?,
                })
            }
        }
    }
}

impl WireEncode for DecodedGeneratedBridgeAtomRoleKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::PrimaryEntry { unit } => encode_value_sum(encoder, 1, unit),
            Self::SignatureDescriptor { unit, signature } => {
                encode_two_value_sum(encoder, 2, unit, signature)
            }
            Self::ContextDescriptor {
                unit,
                context_index,
            } => encode_two_value_sum(encoder, 3, unit, context_index),
            Self::StaticAssertSupport { unit, layout } => {
                encode_two_value_sum(encoder, 4, unit, layout)
            }
        }
    }
}

impl WireDecode for DecodedGeneratedBridgeAtomRoleKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::PrimaryEntry {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::SignatureDescriptor {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                    signature: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ContextDescriptor {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                    context_index: decoder.field(2, CallbackParameterIndex::decode)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::StaticAssertSupport {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                    layout: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedGeneratedBridgeAtomKey {
    producer: DecodedPersistentId<ConeIdentity>,
    atom: DecodedGeneratedBridgeAtomRoleKey,
}

impl DecodedGeneratedBridgeAtomKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<GeneratedBridgeAtomKey, E>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<GeneratedBridgeUnitId, Error = E>
            + PersistentIdResolver<CanonicalCAbiSignatureFingerprint, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        Ok(GeneratedBridgeAtomKey::new(
            resolver.resolve(self.producer)?,
            self.atom.resolve(resolver)?,
        ))
    }
}

impl WireEncode for DecodedGeneratedBridgeAtomKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.producer.encode(encoder)?;
        encoder.field(2)?;
        self.atom.encode(encoder)
    }
}

impl WireDecode for DecodedGeneratedBridgeAtomKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            producer: decoder.field(1, DecodedPersistentId::decode)?,
            atom: decoder.field(2, DecodedGeneratedBridgeAtomRoleKey::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedGeneratedBridgeSemanticTarget {
    unit: DecodedPersistentId<GeneratedBridgeUnitId>,
}

impl DecodedGeneratedBridgeSemanticTarget {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<GeneratedBridgeSemanticTarget, E>
    where
        R: PersistentIdResolver<GeneratedBridgeUnitId, Error = E>,
    {
        resolver
            .resolve(self.unit)
            .map(GeneratedBridgeSemanticTarget::new)
    }
}

impl WireEncode for DecodedGeneratedBridgeSemanticTarget {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        self.unit.encode(encoder)
    }
}

impl WireDecode for DecodedGeneratedBridgeSemanticTarget {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            unit: decoder.field(1, DecodedPersistentId::decode)?,
        })
    }
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
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
