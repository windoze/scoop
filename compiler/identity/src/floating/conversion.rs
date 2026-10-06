//! Numeric conversions retain the integer kind of their own IR stage.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FloatConversion<I> {
    FromInteger {
        source: I,
        target: FloatKind,
    },
    ToInteger {
        source: FloatKind,
        target: I,
    },
    BetweenFloats {
        source: FloatKind,
        target: FloatKind,
    },
}

impl<I> FloatConversion<I> {
    pub fn map_integer<J>(self, map: impl FnOnce(I) -> J) -> FloatConversion<J> {
        match self {
            Self::FromInteger { source, target } => FloatConversion::FromInteger {
                source: map(source),
                target,
            },
            Self::ToInteger { source, target } => FloatConversion::ToInteger {
                source,
                target: map(target),
            },
            Self::BetweenFloats { source, target } => {
                FloatConversion::BetweenFloats { source, target }
            }
        }
    }
}

impl<I: WireEncode> WireEncode for FloatConversion<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        match self {
            Self::FromInteger { source, target } => {
                encoder.unsigned(1)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::ToInteger { source, target } => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::BetweenFloats { source, target } => {
                encoder.unsigned(3)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
        }
    }
}

impl<I: WireDecode> WireDecode for FloatConversion<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::FromInteger {
                source: decoder.field(1, I::decode)?,
                target: decoder.field(2, FloatKind::decode)?,
            }),
            2 => Ok(Self::ToInteger {
                source: decoder.field(1, FloatKind::decode)?,
                target: decoder.field(2, I::decode)?,
            }),
            3 => Ok(Self::BetweenFloats {
                source: decoder.field(1, FloatKind::decode)?,
                target: decoder.field(2, FloatKind::decode)?,
            }),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
