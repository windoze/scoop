use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{ConstPropertyValue, HirIntegerConstant, IntegerKind};

mod property_closure;
mod record;
mod table;

pub use property_closure::ExportConstValueClosureValidationError;
pub use record::{
    ConstPropertyDeclarationSourceV1, DecodedExportConstValueV1, ExportConstValueResolutionError,
    ExportConstValueResolver, ExportConstValueSemanticAuthority,
    ExportConstValueSemanticValidationError, ExportConstValueV1,
};
pub use table::{
    CanonicalExportConstValuesV1, DecodedCanonicalExportConstValuesV1,
    ExportConstValueSetBuildError, ExportConstValueSetSemanticValidationError,
    ExportConstValueSetValidationError,
};

/// A width-exact integer constant in the cross-Cone interface.
///
/// Signed payloads preserve their source-width two's-complement bits. Keeping
/// this type separate from the local HIR value prevents its versioned wire
/// contract from becoming an accidental property of the compiler arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalIntegerConstantV1 {
    Signed8(u8),
    Signed16(u16),
    Signed32(u32),
    Signed64(u64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
}

impl CanonicalIntegerConstantV1 {
    pub const fn kind(self) -> IntegerKind {
        match self {
            Self::Signed8(_) => IntegerKind::SIGNED_8,
            Self::Signed16(_) => IntegerKind::SIGNED_16,
            Self::Signed32(_) => IntegerKind::SIGNED_32,
            Self::Signed64(_) => IntegerKind::SIGNED_64,
            Self::Unsigned8(_) => IntegerKind::UNSIGNED_8,
            Self::Unsigned16(_) => IntegerKind::UNSIGNED_16,
            Self::Unsigned32(_) => IntegerKind::UNSIGNED_32,
            Self::Unsigned64(_) => IntegerKind::UNSIGNED_64,
        }
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::Signed8(value) | Self::Unsigned8(value) => value as u64,
            Self::Signed16(value) | Self::Unsigned16(value) => value as u64,
            Self::Signed32(value) | Self::Unsigned32(value) => value as u64,
            Self::Signed64(value) | Self::Unsigned64(value) => value,
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::Signed8(_) => 1,
            Self::Signed16(_) => 2,
            Self::Signed32(_) => 3,
            Self::Signed64(_) => 4,
            Self::Unsigned8(_) => 5,
            Self::Unsigned16(_) => 6,
            Self::Unsigned32(_) => 7,
            Self::Unsigned64(_) => 8,
        }
    }
}

impl WireEncode for CanonicalIntegerConstantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(self.tag())?;
        encoder.field(1)?;
        encoder.unsigned(self.raw_bits())
    }
}

impl WireDecode for CanonicalIntegerConstantV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder.field(1, decode_u8).map(Self::Signed8),
            2 => decoder.field(1, decode_u16).map(Self::Signed16),
            3 => decoder.field(1, decode_u32).map(Self::Signed32),
            4 => decoder.field(1, Decoder::unsigned).map(Self::Signed64),
            5 => decoder.field(1, decode_u8).map(Self::Unsigned8),
            6 => decoder.field(1, decode_u16).map(Self::Unsigned16),
            7 => decoder.field(1, decode_u32).map(Self::Unsigned32),
            8 => decoder.field(1, Decoder::unsigned).map(Self::Unsigned64),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl From<HirIntegerConstant> for CanonicalIntegerConstantV1 {
    fn from(value: HirIntegerConstant) -> Self {
        match value {
            HirIntegerConstant::Signed8(raw) => Self::Signed8(raw),
            HirIntegerConstant::Signed16(raw) => Self::Signed16(raw),
            HirIntegerConstant::Signed32(raw) => Self::Signed32(raw),
            HirIntegerConstant::Signed64(raw) => Self::Signed64(raw),
            HirIntegerConstant::Unsigned8(raw) => Self::Unsigned8(raw),
            HirIntegerConstant::Unsigned16(raw) => Self::Unsigned16(raw),
            HirIntegerConstant::Unsigned32(raw) => Self::Unsigned32(raw),
            HirIntegerConstant::Unsigned64(raw) => Self::Unsigned64(raw),
        }
    }
}

impl From<CanonicalIntegerConstantV1> for HirIntegerConstant {
    fn from(value: CanonicalIntegerConstantV1) -> Self {
        match value {
            CanonicalIntegerConstantV1::Signed8(raw) => Self::Signed8(raw),
            CanonicalIntegerConstantV1::Signed16(raw) => Self::Signed16(raw),
            CanonicalIntegerConstantV1::Signed32(raw) => Self::Signed32(raw),
            CanonicalIntegerConstantV1::Signed64(raw) => Self::Signed64(raw),
            CanonicalIntegerConstantV1::Unsigned8(raw) => Self::Unsigned8(raw),
            CanonicalIntegerConstantV1::Unsigned16(raw) => Self::Unsigned16(raw),
            CanonicalIntegerConstantV1::Unsigned32(raw) => Self::Unsigned32(raw),
            CanonicalIntegerConstantV1::Unsigned64(raw) => Self::Unsigned64(raw),
        }
    }
}

/// An explicit unsigned enumeration used instead of CBOR's native booleans.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalBooleanV1 {
    False,
    True,
}

impl CanonicalBooleanV1 {
    pub const fn value(self) -> bool {
        matches!(self, Self::True)
    }
}

impl WireEncode for CanonicalBooleanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::False => 1,
            Self::True => 2,
        })
    }
}

impl WireDecode for CanonicalBooleanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::False),
            2 => Ok(Self::True),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl From<bool> for CanonicalBooleanV1 {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

impl From<CanonicalBooleanV1> for bool {
    fn from(value: CanonicalBooleanV1) -> Self {
        value.value()
    }
}

/// A compile-time value that may cross a Cone boundary in schema version 1.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalConstValueV1 {
    Integer(CanonicalIntegerConstantV1),
    Boolean(CanonicalBooleanV1),
    String(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalConstValueKindV1 {
    Integer(IntegerKind),
    Boolean,
    String,
}

impl CanonicalConstValueV1 {
    pub const fn kind(&self) -> CanonicalConstValueKindV1 {
        match self {
            Self::Integer(value) => CanonicalConstValueKindV1::Integer(value.kind()),
            Self::Boolean(_) => CanonicalConstValueKindV1::Boolean,
            Self::String(_) => CanonicalConstValueKindV1::String,
        }
    }
}

impl WireEncode for CanonicalConstValueV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Integer(_) => 1,
            Self::Boolean(_) => 2,
            Self::String(_) => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Integer(value) => value.encode(encoder),
            Self::Boolean(value) => value.encode(encoder),
            Self::String(value) => encoder.text(value),
        }
    }
}

impl WireDecode for CanonicalConstValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, CanonicalIntegerConstantV1::decode)
                .map(Self::Integer),
            2 => decoder
                .field(1, CanonicalBooleanV1::decode)
                .map(Self::Boolean),
            3 => decoder.field(1, Decoder::owned_text).map(Self::String),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl From<ConstPropertyValue> for CanonicalConstValueV1 {
    fn from(value: ConstPropertyValue) -> Self {
        match value {
            ConstPropertyValue::Integer(value) => Self::Integer(value.into()),
            ConstPropertyValue::Boolean(value) => Self::Boolean(value.into()),
            ConstPropertyValue::String(value) => Self::String(value),
        }
    }
}

impl From<CanonicalConstValueV1> for ConstPropertyValue {
    fn from(value: CanonicalConstValueV1) -> Self {
        match value {
            CanonicalConstValueV1::Integer(value) => Self::Integer(value.into()),
            CanonicalConstValueV1::Boolean(value) => Self::Boolean(value.into()),
            CanonicalConstValueV1::String(value) => Self::String(value),
        }
    }
}

fn decode_u8(decoder: &mut Decoder<'_>) -> Result<u8, WireError> {
    decode_narrow_unsigned(decoder)
}

fn decode_u16(decoder: &mut Decoder<'_>) -> Result<u16, WireError> {
    decode_narrow_unsigned(decoder)
}

fn decode_u32(decoder: &mut Decoder<'_>) -> Result<u32, WireError> {
    decode_narrow_unsigned(decoder)
}

fn decode_narrow_unsigned<T>(decoder: &mut Decoder<'_>) -> Result<T, WireError>
where
    T: TryFrom<u64>,
{
    let value = decoder.unsigned()?;
    T::try_from(value).map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))
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

#[cfg(test)]
mod tests;
