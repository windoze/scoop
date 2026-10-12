use scoop_wire::{Encoder, WireEncode};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NativeSymbolNormalization {
    MachOExternalUnderscore,
    ElfIdentity,
}

impl NativeSymbolNormalization {
    pub const fn external_prefix(self) -> &'static [u8] {
        match self {
            Self::MachOExternalUnderscore => b"_",
            Self::ElfIdentity => b"",
        }
    }

    /// Normalize a canonical compiler-generated logical symbol to the bytes
    /// used by the target object format. Unlike ordinary C identifiers, a C
    /// `asm` label names these bytes directly and therefore must consume this
    /// target projection explicitly.
    pub fn compiler_generated_object_symbol(self, logical_symbol: &str) -> String {
        match self {
            Self::MachOExternalUnderscore => format!("_{logical_symbol}"),
            Self::ElfIdentity => logical_symbol.to_owned(),
        }
    }
}

impl WireEncode for NativeSymbolNormalization {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::MachOExternalUnderscore => 1,
            Self::ElfIdentity => 2,
        })
    }
}
