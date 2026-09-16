use std::num::NonZeroU32;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableOperatorV1 {
    UnaryPlus,
    UnaryMinus,
    Not,
    Inc,
    Dec,
    Plus,
    Minus,
    Times,
    Div,
    Rem,
    RangeTo,
    RangeUntil,
    Contains,
    Get,
    Set,
    Invoke,
    PlusAssign,
    MinusAssign,
    TimesAssign,
    DivAssign,
    RemAssign,
    CompareTo,
    Equals,
    Component { index: NonZeroU32 },
    Iterator,
}

impl CallableOperatorV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::UnaryPlus => 1,
            Self::UnaryMinus => 2,
            Self::Not => 3,
            Self::Inc => 4,
            Self::Dec => 5,
            Self::Plus => 6,
            Self::Minus => 7,
            Self::Times => 8,
            Self::Div => 9,
            Self::Rem => 10,
            Self::RangeTo => 11,
            Self::RangeUntil => 12,
            Self::Contains => 13,
            Self::Get => 14,
            Self::Set => 15,
            Self::Invoke => 16,
            Self::PlusAssign => 17,
            Self::MinusAssign => 18,
            Self::TimesAssign => 19,
            Self::DivAssign => 20,
            Self::RemAssign => 21,
            Self::CompareTo => 22,
            Self::Equals => 23,
            Self::Component { .. } => 24,
            Self::Iterator => 25,
        }
    }
}

impl WireEncode for CallableOperatorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Component { index } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(self.tag())?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(index.get()))
            }
            operator => encode_empty_sum(encoder, operator.tag()),
        }
    }
}

impl WireDecode for CallableOperatorV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let operator = match tag {
            1 => Self::UnaryPlus,
            2 => Self::UnaryMinus,
            3 => Self::Not,
            4 => Self::Inc,
            5 => Self::Dec,
            6 => Self::Plus,
            7 => Self::Minus,
            8 => Self::Times,
            9 => Self::Div,
            10 => Self::Rem,
            11 => Self::RangeTo,
            12 => Self::RangeUntil,
            13 => Self::Contains,
            14 => Self::Get,
            15 => Self::Set,
            16 => Self::Invoke,
            17 => Self::PlusAssign,
            18 => Self::MinusAssign,
            19 => Self::TimesAssign,
            20 => Self::DivAssign,
            21 => Self::RemAssign,
            22 => Self::CompareTo,
            23 => Self::Equals,
            24 => {
                expect_sum_length(decoder, fields, 2)?;
                let index = decoder.field(1, Decoder::u32)?;
                return NonZeroU32::new(index)
                    .map(|index| Self::Component { index })
                    .ok_or_else(|| integer_out_of_range(decoder));
            }
            25 => Self::Iterator,
            tag => return Err(unknown_tag(decoder, tag)),
        };
        expect_sum_length(decoder, fields, 1)?;
        Ok(operator)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyDelegateOperatorV1 {
    ProvideDelegate,
    GetValue,
    SetValue,
}

impl WireEncode for PropertyDelegateOperatorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ProvideDelegate => 1,
            Self::GetValue => 2,
            Self::SetValue => 3,
        })
    }
}

impl WireDecode for PropertyDelegateOperatorV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ProvideDelegate),
            2 => Ok(Self::GetValue),
            3 => Ok(Self::SetValue),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableOperatorRoleV1 {
    None,
    Language(CallableOperatorV1),
    PropertyDelegate(PropertyDelegateOperatorV1),
}

impl WireEncode for CallableOperatorRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => encode_empty_sum(encoder, 1),
            Self::Language(operator) => encode_value_sum(encoder, 2, operator),
            Self::PropertyDelegate(operator) => encode_value_sum(encoder, 3, operator),
        }
    }
}

impl WireDecode for CallableOperatorRoleV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::None)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, CallableOperatorV1::decode)
                    .map(Self::Language)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, PropertyDelegateOperatorV1::decode)
                    .map(Self::PropertyDelegate)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn integer_out_of_range(decoder: &Decoder<'_, '_>) -> WireError {
    WireError::new(
        WireErrorKind::IntegerOutOfRange,
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
