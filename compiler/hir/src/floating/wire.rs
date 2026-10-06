use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for FloatIntrinsicKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::Conversion(_)) {
            2
        } else {
            3
        })?;
        encoder.field(0)?;
        match self {
            Self::Unary { kind, operation } => {
                encoder.unsigned(1)?;
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                operation.encode(encoder)
            }
            Self::Binary { kind, operation } => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                operation.encode(encoder)
            }
            Self::Conversion(conversion) => {
                encoder.unsigned(3)?;
                encoder.field(1)?;
                conversion
                    .map_integer(DefaultIntegerKindV1::from)
                    .encode(encoder)
            }
        }
    }
}

impl WireDecode for FloatIntrinsicKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = if tag == 3 { 2 } else { 3 };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            1 => Ok(Self::Unary {
                kind: decoder.field(1, FloatKind::decode)?,
                operation: decoder.field(2, FloatUnaryOperator::decode)?,
            }),
            2 => Ok(Self::Binary {
                kind: decoder.field(1, FloatKind::decode)?,
                operation: decoder.field(2, FloatBinaryOperator::decode)?,
            }),
            3 => Ok(Self::Conversion(
                decoder
                    .field(1, DefaultFloatConversionV1::decode)?
                    .map_integer(IntegerKind::from),
            )),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
