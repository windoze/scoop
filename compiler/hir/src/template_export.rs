//! Producer selection for optional cross-Cone optimization bodies.

use std::fmt;
use std::str::FromStr;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum IrTemplateExport {
    Off,
    #[default]
    Default,
    Full,
}

impl IrTemplateExport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Default => "default",
            Self::Full => "full",
        }
    }
}

impl fmt::Display for IrTemplateExport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for IrTemplateExport {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "off" => Ok(Self::Off),
            "default" => Ok(Self::Default),
            "full" => Ok(Self::Full),
            _ => Err("expected off, default or full"),
        }
    }
}

impl WireEncode for IrTemplateExport {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Off => 1,
            Self::Default => 2,
            Self::Full => 3,
        })
    }
}

impl WireDecode for IrTemplateExport {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Off),
            2 => Ok(Self::Default),
            3 => Ok(Self::Full),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
