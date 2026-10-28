//! Native debug production settings, independent of optimization and ABI.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DebugInfoLevel {
    #[default]
    None,
    LineTables,
}

impl DebugInfoLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LineTables => "line-tables",
        }
    }
}

impl std::str::FromStr for DebugInfoLevel {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "line-tables" => Ok(Self::LineTables),
            _ => Err("expected none or line-tables"),
        }
    }
}

impl std::fmt::Display for DebugInfoLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl WireEncode for DebugInfoLevel {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::None => 0,
            Self::LineTables => 1,
        })
    }
}

impl WireDecode for DebugInfoLevel {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            0 => Ok(Self::None),
            1 => Ok(Self::LineTables),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
