use super::*;

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
