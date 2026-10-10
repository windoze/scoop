use crate::{CanonicalConstValueKindV1, CanonicalConstValueV1, DefaultIntegerKindV1, FloatKind};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[cfg(test)]
mod tests;

/// Static annotation data; arrays never materialize as managed values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalAnnotationValueV1 {
    Scalar(CanonicalConstValueV1),
    Array {
        element_type: CanonicalConstValueKindV1,
        elements: Vec<CanonicalConstValueV1>,
    },
}

impl WireEncode for CanonicalAnnotationValueV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Scalar(value) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
            Self::Array {
                element_type,
                elements,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                element_type.encode(encoder)?;
                encoder.field(2)?;
                super::wire::sequence(encoder, elements)
            }
        }
    }
}

impl WireDecode for CanonicalAnnotationValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, CanonicalConstValueV1::decode)
                    .map(Self::Scalar)
            }
            2 => {
                expect_fields(decoder, fields, 3)?;
                Ok(Self::Array {
                    element_type: decoder.field(1, CanonicalConstValueKindV1::decode)?,
                    elements: decoder.field(2, |d| {
                        d.decode_array(|d, _| CanonicalConstValueV1::decode(d))
                    })?,
                })
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for CanonicalConstValueKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::Integer(_) | Self::Float(_)) {
            2
        } else {
            1
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Integer(_) => 1,
            Self::Boolean => 2,
            Self::String => 3,
            Self::Char => 4,
            Self::Float(_) => 5,
        })?;
        match self {
            Self::Integer(kind) => {
                encoder.field(1)?;
                DefaultIntegerKindV1::from(*kind).encode(encoder)
            }
            Self::Float(kind) => {
                encoder.field(1)?;
                kind.encode(encoder)
            }
            Self::Boolean | Self::String | Self::Char => Ok(()),
        }
    }
}

impl WireDecode for CanonicalConstValueKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_fields(decoder, fields, if matches!(tag, 1 | 5) { 2 } else { 1 })?;
        match tag {
            1 => decoder
                .field(1, DefaultIntegerKindV1::decode)
                .map(|kind| Self::Integer(kind.into())),
            2 => Ok(Self::Boolean),
            3 => Ok(Self::String),
            4 => Ok(Self::Char),
            5 => decoder.field(1, FloatKind::decode).map(Self::Float),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn expect_fields(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
