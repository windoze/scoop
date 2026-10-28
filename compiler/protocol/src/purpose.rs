//! The current Cone's compilation purpose, independent of its source kind.

use scoop_wire::cbor::{Decoder, Encoder};
use scoop_wire::{WireDecode, WireEncode, WireError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompilationPurpose {
    #[default]
    Production,
    Test,
}

impl WireEncode for CompilationPurpose {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Production => 1,
            Self::Test => 2,
        })
    }
}

impl WireDecode for CompilationPurpose {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Production),
            2 => Ok(Self::Test),
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}
