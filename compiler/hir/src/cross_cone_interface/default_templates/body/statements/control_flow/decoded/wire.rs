use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;

impl WireEncode for DecodedDefaultWhenV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.arms)?;
        encoder.field(3)?;
        self.fallback.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultWhenV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            subject: decoder.field(1, DecodedDefaultExpressionV1::decode)?,
            arms: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultWhenArmV1::decode(decoder))
            })?,
            fallback: decoder.field(3, DecodedDefaultWhenFallbackV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultWhenArmV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.pattern.encode(encoder)?;
        encoder.field(2)?;
        self.guard.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultWhenArmV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            pattern: decoder.field(1, DecodedDefaultPatternV1::decode)?,
            guard: decoder.field(2, DecodedOptionalDefaultWhenGuardV1::decode)?,
            body: decoder.field(3, decode_statements)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedOptionalDefaultWhenGuardV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(guard) => encode_one(encoder, 2, guard.as_ref()),
        }
    }
}

impl WireDecode for DecodedOptionalDefaultWhenGuardV1 {
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
                    .field(1, DecodedDefaultWhenGuardV1::decode)
                    .map(Box::new)
                    .map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedDefaultWhenGuardV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.setup)?;
        encoder.field(2)?;
        self.condition.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultWhenGuardV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            setup: decoder.field(1, decode_statements)?,
            condition: decoder.field(2, DecodedDefaultExpressionV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultWhenFallbackV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Else(statements) => encode_one(encoder, 1, &WireSequence(statements)),
            Self::IrrefutableArm { subject_type } => encode_one(encoder, 2, subject_type),
            Self::PatternMatrix { subject_type } => encode_one(encoder, 3, subject_type),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => encode_two(encoder, 4, subject_type, owner_type),
        }
    }
}

impl WireDecode for DecodedDefaultWhenFallbackV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder.field(1, decode_statements).map(Self::Else)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(|subject_type| Self::IrrefutableArm { subject_type })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(|subject_type| Self::PatternMatrix { subject_type })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::EnumPatternMatrix {
                    subject_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    owner_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedDefaultTryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.catches)?;
        encoder.field(3)?;
        self.finally_body.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultTryV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            body: decoder.field(1, decode_statements)?,
            catches: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultCatchV1::decode(decoder))
            })?,
            finally_body: decoder.field(3, DecodedOptionalDefaultStatementListV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultCatchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.local_index))?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultCatchV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            local_index: decoder.field(1, Decoder::u32)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            body: decoder.field(3, decode_statements)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedOptionalDefaultStatementListV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(statements) => encode_one(encoder, 2, &WireSequence(statements)),
        }
    }
}

impl WireDecode for DecodedOptionalDefaultStatementListV1 {
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
                decoder.field(1, decode_statements).map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

struct WireSequence<'a, T>(&'a [T]);

impl<T: WireEncode> WireEncode for WireSequence<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, self.0)
    }
}

fn decode_statements(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedDefaultStatementV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_empty(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_one(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two(
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

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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
