//! COFF import interfaces and ordinary PE linker default-library resolution.
use super::*;
use crate::native_input::NativeContent;
use std::collections::BTreeSet;

pub(crate) enum CoffBinding {
    Import {
        input: NativeInputId,
        dll: String,
        definition: NativeSymbolDefinition,
    },
    /// Long-format .idata members must retain their original archive grouping.
    LongImport {
        input: NativeInputId,
        definition: NativeSymbolDefinition,
    },
    /// CRT defaults, weak aliases and .drectve are resolved by the real linker.
    Linker,
}

#[derive(Default)]
pub(crate) struct CoffNamespace {
    pub bindings: BTreeMap<String, CoffBinding>,
}

impl CoffNamespace {
    pub fn candidates(
        &self,
        native: &NativeInputs,
        symbol: &str,
        explicit: Option<&[NativeInputId]>,
    ) -> Vec<NativeBinding> {
        native
            .ordered_files()
            .into_iter()
            .filter(|file| explicit.is_none_or(|ids| ids.contains(&file.id)))
            .filter_map(|file| {
                let NativeContent::Archive(members) = &file.content else {
                    return None;
                };
                members.iter().find_map(|member| {
                    if let Some(import) = member.import() {
                        import.definition(symbol).map(|definition| {
                            NativeBinding::Coff(CoffBinding::Import {
                                input: file.id,
                                dll: import.dll.clone(),
                                definition,
                            })
                        })
                    } else if member.has_imports() {
                        let info = member.object_index()?;
                        let definition = info.definitions.get(symbol).copied().or_else(|| {
                            info.definitions
                                .get(&format!("__imp_{symbol}"))
                                .map(|definition| NativeSymbolDefinition {
                                    read_only: false,
                                    ..*definition
                                })
                        })?;
                        Some(NativeBinding::Coff(CoffBinding::LongImport {
                            input: file.id,
                            definition,
                        }))
                    } else {
                        None
                    }
                })
            })
            // Imports are lazy archive choices, not extracted strong objects.
            // Windows SDK libraries can expose the same name from different DLLs
            // (e.g. kernel32 and ntdll). Keep the native linker's input order;
            // an explicit source library still restricts the candidates above.
            .take(if explicit.is_none() { 1 } else { usize::MAX })
            .collect()
    }

    pub fn selected(&self) -> BTreeSet<NativeInputId> {
        self.bindings
            .values()
            .filter_map(|binding| match binding {
                CoffBinding::Import { input, .. } | CoffBinding::LongImport { input, .. } => {
                    Some(*input)
                }
                CoffBinding::Linker => None,
            })
            .collect()
    }

    pub fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        use scoop_wire::WireEncode;
        e.array(2)?;
        e.unsigned(3)?;
        e.array(self.bindings.len() as u64)?;
        for (symbol, binding) in &self.bindings {
            e.array(2)?;
            e.text(symbol)?;
            match binding {
                CoffBinding::Import { input, dll, .. } => {
                    e.array(2)?;
                    input.encode(e)?;
                    e.text(dll)?;
                }
                CoffBinding::LongImport { input, .. } => {
                    e.array(1)?;
                    input.encode(e)?;
                }
                CoffBinding::Linker => e.array(0)?,
            }
        }
        Ok(())
    }

    pub fn dump(&self) -> String {
        self.bindings
            .iter()
            .map(|(symbol, binding)| match binding {
                CoffBinding::Import { input, dll, .. } => {
                    format!("PE import {symbol} library={input} dll={dll}\n")
                }
                CoffBinding::LongImport { input, .. } => {
                    format!("PE archive import {symbol} library={input}\n")
                }
                CoffBinding::Linker => format!("PE linker reference {symbol}\n"),
            })
            .collect()
    }
}
