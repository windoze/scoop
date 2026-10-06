//! The two IEEE scalar formats shared by IR and artifact records.

use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatKind {
    F32,
    F64,
}

impl FloatKind {
    pub const ALL: [Self; 2] = [Self::F32, Self::F64];

    pub const fn bits(self) -> u32 {
        match self {
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }

    pub const fn bytes(self) -> u32 {
        self.bits() / 8
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::F32 => "Float",
            Self::F64 => "Double",
        }
    }

    pub const fn registry_key(self) -> &'static str {
        match self {
            Self::F32 => "float",
            Self::F64 => "double",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::F32 => "core_float",
            Self::F64 => "core_double",
        }
    }
}

/// Compiler data equality compares bits, independently of source IEEE equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatConstant {
    F32(u32),
    F64(u64),
}

impl FloatConstant {
    pub const fn kind(self) -> FloatKind {
        match self {
            Self::F32(_) => FloatKind::F32,
            Self::F64(_) => FloatKind::F64,
        }
    }

    pub const fn raw_bits(self) -> u64 {
        match self {
            Self::F32(bits) => bits as u64,
            Self::F64(bits) => bits,
        }
    }

    pub const fn negate(self) -> Self {
        match self {
            Self::F32(bits) => Self::F32(bits ^ (1 << 31)),
            Self::F64(bits) => Self::F64(bits ^ (1 << 63)),
        }
    }
}

impl fmt::Display for FloatConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::F32(bits) => write!(f, "f32(0x{bits:08x})"),
            Self::F64(bits) => write!(f, "f64(0x{bits:016x})"),
        }
    }
}

impl WireEncode for FloatKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.bits()))
    }
}

impl WireDecode for FloatKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            32 => Ok(Self::F32),
            64 => Ok(Self::F64),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for FloatConstant {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind().encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.raw_bits())
    }
}

impl WireDecode for FloatConstant {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = decoder.map()?;
        if actual != 2 {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual,
                },
            ));
        }
        let kind = decoder.field(1, FloatKind::decode)?;
        let bits = decoder.field(2, Decoder::unsigned)?;
        match kind {
            FloatKind::F32 => u32::try_from(bits)
                .map(Self::F32)
                .map_err(|_| error(decoder, WireErrorKind::UnknownTag { tag: bits })),
            FloatKind::F64 => Ok(Self::F64(bits)),
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_wire::{decode_canonical, encode};

    #[test]
    fn wire_preserves_zero_sign_and_nan_payloads() {
        for value in [
            FloatConstant::F32(0),
            FloatConstant::F32(0x8000_0000),
            FloatConstant::F32(0x7f80_0001),
            FloatConstant::F32(0xffc1_2345),
            FloatConstant::F64(0x8000_0000_0000_0000),
            FloatConstant::F64(0x7ff0_0000_0000_0001),
            FloatConstant::F64(0xfff8_0123_4567_89ab),
        ] {
            assert_eq!(
                decode_canonical::<FloatConstant>(&encode(&value).unwrap()).unwrap(),
                value
            );
            assert_ne!(value, value.negate());
            assert_eq!(value, value.negate().negate());
        }
    }

    #[test]
    fn wire_rejects_unknown_formats_and_oversized_single_payloads() {
        assert!(decode_canonical::<FloatKind>(&[0x18, 0x80]).is_err());
        let oversized = [
            0xa2, 0x01, 0x18, 0x20, 0x02, 0x1b, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(decode_canonical::<FloatConstant>(&oversized).is_err());
    }
}
