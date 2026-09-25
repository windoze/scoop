use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HirExpressionTypeRoleV1 {
    Value,
    SizeOf,
    AlignOf,
    TypeTest,
    ArrayElement,
    BoxedValue,
}

impl HirExpressionTypeRoleV1 {
    pub const fn requires_shape_support(self) -> bool {
        matches!(self, Self::TypeTest | Self::BoxedValue)
    }
}

impl WireEncode for HirExpressionTypeRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Value => 1,
            Self::SizeOf => 2,
            Self::AlignOf => 3,
            Self::TypeTest => 4,
            Self::ArrayElement => 5,
            Self::BoxedValue => 6,
        })
    }
}

impl WireDecode for HirExpressionTypeRoleV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Value),
            2 => Ok(Self::SizeOf),
            3 => Ok(Self::AlignOf),
            4 => Ok(Self::TypeTest),
            5 => Ok(Self::ArrayElement),
            6 => Ok(Self::BoxedValue),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
