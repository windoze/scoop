use super::*;
use crate::{NativeSymbolBinding, NativeSymbolDefinition, NativeSymbolKind};
use object::read::coff::{ImportFile, ImportType};

/// A short import's public interface; the original member carries DLL name,
/// export spelling or ordinal to the native linker without reconstruction.
pub(crate) struct CoffImport {
    pub dll: String,
    pub symbol: String,
    pub definition: NativeSymbolDefinition,
}

impl CoffImport {
    pub fn read(bytes: &[u8]) -> Result<Self, LinkError> {
        let import = ImportFile::parse(bytes).map_err(error)?;
        if import.architecture() != object::Architecture::X86_64 {
            return Err(error("native import must target AMD64"));
        }
        Ok(Self {
            dll: std::str::from_utf8(import.dll()).map_err(error)?.to_owned(),
            symbol: std::str::from_utf8(import.symbol())
                .map_err(error)?
                .to_owned(),
            definition: NativeSymbolDefinition {
                kind: if import.import_type() == ImportType::Code {
                    NativeSymbolKind::Function
                } else {
                    NativeSymbolKind::Data
                },
                read_only: import.import_type() != ImportType::Data,
                binding: NativeSymbolBinding::Strong,
            },
        })
    }

    pub fn definition(&self, symbol: &str) -> Option<NativeSymbolDefinition> {
        if symbol == self.symbol {
            Some(self.definition)
        } else if symbol.strip_prefix("__imp_") == Some(self.symbol.as_str()) {
            Some(NativeSymbolDefinition {
                kind: NativeSymbolKind::Data,
                read_only: true,
                binding: NativeSymbolBinding::Strong,
            })
        } else {
            None
        }
    }
}
