//! Closed producer settings; these never participate in entity or ABI identity.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum OptimizationMode {
    #[default]
    Debug,
    Release,
}

impl OptimizationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    pub const fn c_flag(self) -> &'static str {
        match self {
            Self::Debug => "-O0",
            Self::Release => "-O2",
        }
    }
}

impl WireEncode for OptimizationMode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Debug => 1,
            Self::Release => 2,
        })
    }
}

impl WireDecode for OptimizationMode {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Debug),
            2 => Ok(Self::Release),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
