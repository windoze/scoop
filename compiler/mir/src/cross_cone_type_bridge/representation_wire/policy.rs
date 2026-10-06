use super::*;
use crate::{IntegerKind, IntegerSignedness, IntegerWidth, MirCLayoutContract, MirCLayoutValue};

impl WireEncode for MirTypeCLayoutPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Ordinary => tag(encoder, 1, 1),
            Self::CLayout(contract) => {
                tag(encoder, 3, 2)?;
                encoder.field(1)?;
                alignment(encoder, contract.aligned)?;
                encoder.field(2)?;
                alignment(encoder, contract.packed)
            }
        }
    }
}
impl WireDecode for MirTypeCLayoutPolicyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                fields(decoder, count, 1)?;
                Ok(Self::Ordinary)
            }
            2 => {
                fields(decoder, count, 3)?;
                Ok(Self::CLayout(MirCLayoutContract {
                    aligned: decoder.field(1, decode_alignment)?,
                    packed: decoder.field(2, decode_alignment)?,
                }))
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
fn alignment(
    encoder: &mut Encoder,
    value: MirCLayoutValue,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(
        encoder,
        1,
        match value {
            MirCLayoutValue::Natural => 1,
            MirCLayoutValue::A1 => 2,
            MirCLayoutValue::A2 => 3,
            MirCLayoutValue::A4 => 4,
            MirCLayoutValue::A8 => 5,
            MirCLayoutValue::A16 => 6,
        },
    )
}
fn decode_alignment(decoder: &mut Decoder<'_>) -> Result<MirCLayoutValue, WireError> {
    decoder.expect_map(1)?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => Ok(MirCLayoutValue::Natural),
        2 => Ok(MirCLayoutValue::A1),
        3 => Ok(MirCLayoutValue::A2),
        4 => Ok(MirCLayoutValue::A4),
        5 => Ok(MirCLayoutValue::A8),
        6 => Ok(MirCLayoutValue::A16),
        tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}
impl WireEncode for MirParamFreeIntrinsicV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unit => tag(encoder, 1, 1),
            Self::Boolean => tag(encoder, 1, 3),
            Self::String => tag(encoder, 1, 4),
            Self::Char => tag(encoder, 1, 5),
            Self::Float(kind) => {
                tag(encoder, 2, 6)?;
                encoder.field(1)?;
                kind.encode(encoder)
            }
            Self::Integer(kind) => {
                tag(encoder, 3, 2)?;
                encoder.field(1)?;
                encoder.unsigned(match kind.signedness() {
                    IntegerSignedness::Signed => 1,
                    IntegerSignedness::Unsigned => 2,
                })?;
                encoder.field(2)?;
                encoder.unsigned(match kind.width() {
                    IntegerWidth::W8 => 1,
                    IntegerWidth::W16 => 2,
                    IntegerWidth::W32 => 3,
                    IntegerWidth::W64 => 4,
                })
            }
        }
    }
}
impl WireDecode for MirParamFreeIntrinsicV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        let kind = decoder.field(0, Decoder::unsigned)?;
        fields(
            decoder,
            count,
            match kind {
                2 => 3,
                6 => 2,
                _ => 1,
            },
        )?;
        match kind {
            1 => Ok(Self::Unit),
            3 => Ok(Self::Boolean),
            4 => Ok(Self::String),
            5 => Ok(Self::Char),
            6 => Ok(Self::Float(decoder.field(1, crate::FloatKind::decode)?)),
            2 => {
                let signedness = match decoder.field(1, Decoder::unsigned)? {
                    1 => IntegerSignedness::Signed,
                    2 => IntegerSignedness::Unsigned,
                    tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
                };
                let width = match decoder.field(2, Decoder::unsigned)? {
                    1 => IntegerWidth::W8,
                    2 => IntegerWidth::W16,
                    3 => IntegerWidth::W32,
                    4 => IntegerWidth::W64,
                    tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
                };
                Ok(Self::Integer(IntegerKind::new(signedness, width)))
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for MirClassKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        tag(
            encoder,
            1,
            match self {
                Self::Final => 1,
                Self::Open => 2,
                Self::Abstract => 3,
            },
        )
    }
}
impl WireDecode for MirClassKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Final),
            2 => Ok(Self::Open),
            3 => Ok(Self::Abstract),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
