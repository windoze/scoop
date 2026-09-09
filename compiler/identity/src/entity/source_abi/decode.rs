use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallbackMode, GcEffect, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention,
    SourceExternFunctionAbi, SourceNativeLibraryBinding, SourceScoopAbiFunctionSignature,
};
use crate::{
    CanonicalNativeNameError, DecodedCanonicalNativeLibraryName, DecodedSignatureTypeKey,
    PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId, SignatureTypeKey,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceNativeLibraryBinding {
    DefaultNativeNamespace,
    LogicalLibrary(DecodedCanonicalNativeLibraryName),
}

impl DecodedSourceNativeLibraryBinding {
    pub fn validate(self) -> Result<SourceNativeLibraryBinding, CanonicalNativeNameError> {
        match self {
            Self::DefaultNativeNamespace => Ok(SourceNativeLibraryBinding::DefaultNativeNamespace),
            Self::LogicalLibrary(name) => name
                .validate()
                .map(SourceNativeLibraryBinding::LogicalLibrary),
        }
    }
}

impl WireEncode for DecodedSourceNativeLibraryBinding {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::DefaultNativeNamespace => encode_empty_sum(encoder, 1),
            Self::LogicalLibrary(name) => encode_value_sum(encoder, 2, name),
        }
    }
}

impl WireDecode for DecodedSourceNativeLibraryBinding {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::DefaultNativeNamespace)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCanonicalNativeLibraryName::decode)
                    .map(Self::LogicalLibrary)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceCAbiReturn {
    Void,
    Value(DecodedSignatureTypeKey),
}

impl DecodedSourceCAbiReturn {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SourceCAbiReturn, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Void => Ok(SourceCAbiReturn::Void),
            Self::Value(value) => value.resolve(resolver).map(SourceCAbiReturn::Value),
        }
    }
}

impl WireEncode for DecodedSourceCAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Void => encode_empty_sum(encoder, 1),
            Self::Value(value) => encode_value_sum(encoder, 2, value),
        }
    }
}

impl WireDecode for DecodedSourceCAbiReturn {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Void)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(Self::Value)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceCAbiFunctionSignature {
    parameters: Vec<DecodedSignatureTypeKey>,
    result: DecodedSourceCAbiReturn,
}

impl DecodedSourceCAbiFunctionSignature {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SourceCAbiFunctionSignature, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        let parameters = resolve_signatures(self.parameters, resolver)?;
        let result = self.result.resolve(resolver)?;
        Ok(SourceCAbiFunctionSignature::new(parameters, result))
    }
}

impl WireEncode for DecodedSourceCAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

impl WireDecode for DecodedSourceCAbiFunctionSignature {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            parameters: decoder.field(1, decode_signatures)?,
            result: decoder.field(2, DecodedSourceCAbiReturn::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceScoopAbiFunctionSignature {
    parameters: Vec<DecodedSignatureTypeKey>,
    result: DecodedSignatureTypeKey,
}

impl DecodedSourceScoopAbiFunctionSignature {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SourceScoopAbiFunctionSignature, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        let parameters = resolve_signatures(self.parameters, resolver)?;
        let result = self.result.resolve(resolver)?;
        Ok(SourceScoopAbiFunctionSignature::new(parameters, result))
    }
}

impl WireEncode for DecodedSourceScoopAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.parameters)?;
        encoder.field(2)?;
        self.result.encode(encoder)
    }
}

impl WireDecode for DecodedSourceScoopAbiFunctionSignature {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            parameters: decoder.field(1, decode_signatures)?,
            result: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceExternFunctionAbi {
    C(DecodedSourceCAbiFunctionSignature),
    Scoop {
        signature: DecodedSourceScoopAbiFunctionSignature,
        gc_effect: GcEffect,
    },
}

impl DecodedSourceExternFunctionAbi {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SourceExternFunctionAbi, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::C(signature) => signature.resolve(resolver).map(SourceExternFunctionAbi::C),
            Self::Scoop {
                signature,
                gc_effect,
            } => Ok(SourceExternFunctionAbi::Scoop {
                signature: signature.resolve(resolver)?,
                gc_effect,
            }),
        }
    }
}

impl WireEncode for DecodedSourceExternFunctionAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::C(signature) => encode_value_sum(encoder, 1, signature),
            Self::Scoop {
                signature,
                gc_effect,
            } => encode_two_value_sum(encoder, 2, signature, gc_effect),
        }
    }
}

impl WireDecode for DecodedSourceExternFunctionAbi {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceCAbiFunctionSignature::decode)
                    .map(Self::C)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Scoop {
                    signature: decoder.field(1, DecodedSourceScoopAbiFunctionSignature::decode)?,
                    gc_effect: decoder.field(2, GcEffect::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for GcEffect {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Managed),
            2 => Ok(Self::NoGc),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for SourceCallingConvention {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Cdecl),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for CallbackMode {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Reusable),
            2 => Ok(Self::OneShot),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn resolve_signatures<R, E>(
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

fn decode_signatures(
    decoder: &mut Decoder<'_, '_>,
) -> Result<Vec<DecodedSignatureTypeKey>, WireError> {
    decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
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

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
