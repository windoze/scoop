//! Closed operations for the two supported IEEE representations.

use super::*;

macro_rules! operators {
    ($name:ident { $($variant:ident = $tag:literal => $key:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        #[repr(u8)]
        pub enum $name { $($variant = $tag),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn registry_key(self) -> &'static str {
                match self { $(Self::$variant => $key),+ }
            }
        }
        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.unsigned(*self as u64)
            }
        }
        impl WireDecode for $name {
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                let tag = decoder.unsigned()?;
                Self::ALL.iter().copied().find(|value| *value as u64 == tag)
                    .ok_or_else(|| error(decoder, WireErrorKind::UnknownTag { tag }))
            }
        }
    }
}

operators!(FloatUnaryOperator {
    Plus = 1 => "unary_plus",
    Negate = 2 => "unary_minus",
    Increment = 3 => "inc",
    Decrement = 4 => "dec",
    IsNaN = 5 => "is_nan",
    IsInfinite = 6 => "is_infinite",
    IsFinite = 7 => "is_finite",
});

impl FloatUnaryOperator {
    pub const fn is_predicate(self) -> bool {
        matches!(self, Self::IsNaN | Self::IsInfinite | Self::IsFinite)
    }
}

operators!(FloatBinaryOperator {
    Add = 1 => "plus",
    Subtract = 2 => "minus",
    Multiply = 3 => "times",
    Divide = 4 => "div",
    Remainder = 5 => "rem",
    Equal = 6 => "equals",
    NotEqual = 7 => "not_equal",
    Less = 8 => "less",
    LessEqual = 9 => "less_equal",
    Greater = 10 => "greater",
    GreaterEqual = 11 => "greater_equal",
    TotalOrder = 12 => "total_order",
});

impl FloatBinaryOperator {
    pub const fn is_predicate(self) -> bool {
        !matches!(
            self,
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Remainder
        )
    }

    pub const fn has_source_member(self) -> bool {
        !matches!(
            self,
            Self::NotEqual | Self::Less | Self::LessEqual | Self::Greater | Self::GreaterEqual
        )
    }
}
