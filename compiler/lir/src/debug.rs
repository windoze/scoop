//! Native debug production settings, independent of optimization and ABI.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod types;
pub use types::*;

/// A source variable mapped to the existing logical ABI or local storage.
#[derive(Debug)]
pub struct DebugBinding {
    pub declaration: std::sync::Arc<scoop_source::DebugDeclaration>,
    pub location: std::sync::Arc<scoop_source::DebugLocation>,
    pub ty: scoop_identity::PersistentExactTypeId,
    pub storage: DebugStorage,
    pub managed: bool,
    /// One-based logical source parameter index; inline locals keep their declaration.
    pub parameter: Option<std::num::NonZeroU32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DebugStorage {
    Local(crate::LocalId),
    Parameter(u32),
    /// Observe an existing SSA definition; never allocate storage for it.
    Temporary(crate::TempId),
}

/// Source observation of an existing static storage definition.
#[derive(Debug)]
pub struct DebugGlobal {
    pub name: std::sync::Arc<str>,
    pub location: std::sync::Arc<scoop_source::DebugLocation>,
    pub ty: scoop_identity::PersistentExactTypeId,
}

#[derive(Debug, Default)]
pub enum DebugSignature {
    #[default]
    Generated,
    Source {
        parameters: Vec<scoop_identity::PersistentExactTypeId>,
        result: Option<scoop_identity::PersistentExactTypeId>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DebugInfoLevel {
    #[default]
    None,
    LineTables,
    Full,
}

impl DebugInfoLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LineTables => "line-tables",
            Self::Full => "full",
        }
    }
}

impl std::str::FromStr for DebugInfoLevel {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "line-tables" => Ok(Self::LineTables),
            "full" => Ok(Self::Full),
            _ => Err("expected none, line-tables or full"),
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
            Self::Full => 2,
        })
    }
}

impl WireDecode for DebugInfoLevel {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            0 => Ok(Self::None),
            1 => Ok(Self::LineTables),
            2 => Ok(Self::Full),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
