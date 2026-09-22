//! Closed source representation policy. These bytes are specific to the
//! shared source shape and type-semantics sections, not the native-boundary witness.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::wire;
use crate::{
    HirCLayoutContract, HirCLayoutValue, IntegerKind, IntegerSignedness, IntegerWidth,
    IntrinsicTypeKind,
};

mod intrinsic_contract;
pub use intrinsic_contract::NominalIntrinsicBinderError;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalCLayoutPolicyV1 {
    Ordinary,
    CLayout { contract: HirCLayoutContract },
}

impl NominalCLayoutPolicyV1 {
    pub const fn from_source_contract(contract: Option<HirCLayoutContract>) -> Self {
        match contract {
            None => Self::Ordinary,
            Some(contract) => Self::CLayout { contract },
        }
    }
}

impl WireEncode for NominalCLayoutPolicyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Ordinary => wire::tag(encoder, 1, 1),
            Self::CLayout { contract } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                encoder.map(2)?;
                encoder.field(1)?;
                encode_alignment(contract.aligned, encoder)?;
                encoder.field(2)?;
                encode_alignment(contract.packed, encoder)
            }
        }
    }
}

impl WireDecode for NominalCLayoutPolicyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Ordinary)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder.field(1, |decoder| {
                    decoder.expect_map(2)?;
                    Ok(Self::CLayout {
                        contract: HirCLayoutContract {
                            aligned: decoder.field(1, decode_alignment)?,
                            packed: decoder.field(2, decode_alignment)?,
                        },
                    })
                })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn encode_alignment(
    value: HirCLayoutValue,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    wire::tag(
        encoder,
        1,
        match value {
            HirCLayoutValue::Natural => 1,
            HirCLayoutValue::A1 => 2,
            HirCLayoutValue::A2 => 3,
            HirCLayoutValue::A4 => 4,
            HirCLayoutValue::A8 => 5,
            HirCLayoutValue::A16 => 6,
        },
    )
}

fn decode_alignment(decoder: &mut Decoder<'_, '_>) -> Result<HirCLayoutValue, WireError> {
    decoder.expect_map(1)?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => Ok(HirCLayoutValue::Natural),
        2 => Ok(HirCLayoutValue::A1),
        3 => Ok(HirCLayoutValue::A2),
        4 => Ok(HirCLayoutValue::A4),
        5 => Ok(HirCLayoutValue::A8),
        6 => Ok(HirCLayoutValue::A16),
        tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

/// The semantic family belongs to its source nominal. It does not replace the
/// exact application arguments of Array, MutableArray, Ptr, or FunPtr.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominalIntrinsicRepresentationV1 {
    family: IntrinsicTypeKind,
}

impl NominalIntrinsicRepresentationV1 {
    pub const fn new(family: IntrinsicTypeKind) -> Self {
        Self { family }
    }
    pub const fn family(self) -> IntrinsicTypeKind {
        self.family
    }
}

impl WireEncode for NominalIntrinsicRepresentationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.family {
            IntrinsicTypeKind::Integer(kind) => {
                wire::tag(encoder, 3, 1)?;
                encoder.field(1)?;
                wire::tag(
                    encoder,
                    1,
                    match kind.signedness() {
                        IntegerSignedness::Signed => 1,
                        IntegerSignedness::Unsigned => 2,
                    },
                )?;
                encoder.field(2)?;
                wire::tag(
                    encoder,
                    1,
                    match kind.width() {
                        IntegerWidth::W8 => 1,
                        IntegerWidth::W16 => 2,
                        IntegerWidth::W32 => 3,
                        IntegerWidth::W64 => 4,
                    },
                )
            }
            IntrinsicTypeKind::Boolean => wire::tag(encoder, 1, 2),
            IntrinsicTypeKind::String => wire::tag(encoder, 1, 3),
            IntrinsicTypeKind::Array => wire::tag(encoder, 1, 4),
            IntrinsicTypeKind::MutableArray => wire::tag(encoder, 1, 5),
            IntrinsicTypeKind::Ptr => wire::tag(encoder, 1, 6),
            IntrinsicTypeKind::FunPtr => wire::tag(encoder, 1, 7),
        }
    }
}

impl WireDecode for NominalIntrinsicRepresentationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let family = match tag {
            1 => {
                wire::expect_fields(decoder, fields, 3)?;
                let signedness = decoder.field(1, decode_signedness)?;
                let width = decoder.field(2, decode_width)?;
                IntrinsicTypeKind::Integer(IntegerKind::new(signedness, width))
            }
            2..=7 => {
                wire::expect_fields(decoder, fields, 1)?;
                match tag {
                    2 => IntrinsicTypeKind::Boolean,
                    3 => IntrinsicTypeKind::String,
                    4 => IntrinsicTypeKind::Array,
                    5 => IntrinsicTypeKind::MutableArray,
                    6 => IntrinsicTypeKind::Ptr,
                    7 => IntrinsicTypeKind::FunPtr,
                    _ => return Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
                }
            }
            tag => return Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        Ok(Self::new(family))
    }
}

fn decode_signedness(decoder: &mut Decoder<'_, '_>) -> Result<IntegerSignedness, WireError> {
    decoder.expect_map(1)?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => Ok(IntegerSignedness::Signed),
        2 => Ok(IntegerSignedness::Unsigned),
        tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn decode_width(decoder: &mut Decoder<'_, '_>) -> Result<IntegerWidth, WireError> {
    decoder.expect_map(1)?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => Ok(IntegerWidth::W8),
        2 => Ok(IntegerWidth::W16),
        3 => Ok(IntegerWidth::W32),
        4 => Ok(IntegerWidth::W64),
        tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}
