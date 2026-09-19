//! Untrusted type references registration carriers.

use super::*;

#[derive(Debug)]
pub enum DecodedStrongTypeDescriptorRefV1 {
    Local(DecodedPersistentId<PersistentExactTypeId>),
    CoreExternal(DecodedPersistentId<PersistentExactTypeId>),
}

impl WireEncode for DecodedStrongTypeDescriptorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(exact_type) => encode_value_sum(encoder, 1, exact_type),
            Self::CoreExternal(exact_type) => encode_value_sum(encoder, 2, exact_type),
        }
    }
}

impl WireDecode for DecodedStrongTypeDescriptorRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let exact_type = decoder.field(1, DecodedPersistentId::decode)?;
        match tag {
            1 => Ok(Self::Local(exact_type)),
            2 => Ok(Self::CoreExternal(exact_type)),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub enum DecodedOptionalStrongTypeDescriptorRefV1 {
    Absent,
    Local(DecodedPersistentId<PersistentExactTypeId>),
    CoreExternal(DecodedPersistentId<PersistentExactTypeId>),
}

impl WireEncode for DecodedOptionalStrongTypeDescriptorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 1)?;
                encode_unsigned_field(encoder, 1, 0)
            }
            Self::Local(exact_type) => encode_value_sum(encoder, 2, exact_type),
            Self::CoreExternal(exact_type) => encode_value_sum(encoder, 3, exact_type),
        }
    }
}

impl WireDecode for DecodedOptionalStrongTypeDescriptorRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                let marker = decoder.field(1, Decoder::unsigned)?;
                if marker == 0 {
                    Ok(Self::Absent)
                } else {
                    Err(unknown_tag(decoder, marker))
                }
            }
            2 => Ok(Self::Local(decoder.field(1, DecodedPersistentId::decode)?)),
            3 => Ok(Self::CoreExternal(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub struct DecodedRuntimeFunctionV1 {
    pub(in crate::production::registration_production) family: u64,
    pub(in crate::production::registration_production) function: u64,
}

impl WireEncode for DecodedRuntimeFunctionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_unsigned_field(encoder, 1, self.family)?;
        encode_unsigned_field(encoder, 2, self.function)
    }
}

impl WireDecode for DecodedRuntimeFunctionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            family: decoder.field(1, Decoder::unsigned)?,
            function: decoder.field(2, Decoder::unsigned)?,
        })
    }
}

#[derive(Debug)]
pub enum DecodedStrongTypeDispatchCallableRefV1 {
    Local(DecodedPersistentId<PersistentCallableBodyId>),
    CoreExternal(DecodedPersistentId<PersistentCallableBodyId>),
    Runtime(DecodedRuntimeFunctionV1),
}

impl WireEncode for DecodedStrongTypeDispatchCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(body) => encode_value_sum(encoder, 1, body),
            Self::CoreExternal(body) => encode_value_sum(encoder, 2, body),
            Self::Runtime(function) => encode_value_sum(encoder, 3, function),
        }
    }
}

impl WireDecode for DecodedStrongTypeDispatchCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => Ok(Self::Local(decoder.field(1, DecodedPersistentId::decode)?)),
            2 => Ok(Self::CoreExternal(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            3 => Ok(Self::Runtime(
                decoder.field(1, DecodedRuntimeFunctionV1::decode)?,
            )),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}
