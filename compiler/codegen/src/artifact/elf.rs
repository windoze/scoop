//! ELF container facts for stackmaps and exception tables.

use std::collections::BTreeMap;
use std::path::Path;

use object::{
    Architecture, BinaryFormat, Object, ObjectKind, ObjectSection, ObjectSymbol, RelocationFlags,
    RelocationTarget, SectionIndex, SymbolKind, elf,
};

use crate::statepoint::ExpectedSafepoints;
use crate::{CodegenError, ValidatedBackendProfile};

use super::{ExpectedEh, FunctionRelocation, ObservedSafepoints, TextSection, eh, parse_stackmaps};

pub(super) struct ElfFunction {
    pub(super) symbol: String,
    pub(super) section: SectionIndex,
    pub(super) text: TextSection,
}

pub(super) struct ElfCode {
    pub(super) functions: BTreeMap<String, ElfFunction>,
    by_address: BTreeMap<(usize, u64), String>,
}

impl ElfCode {
    fn read(file: &object::File<'_>) -> Result<Self, CodegenError> {
        let mut functions = BTreeMap::new();
        let mut by_address = BTreeMap::new();
        for symbol in file.symbols() {
            if symbol.kind() != SymbolKind::Text || !symbol.is_definition() {
                continue;
            }
            let name = symbol.name().map_err(error)?.to_string();
            let section_index = symbol
                .section_index()
                .ok_or_else(|| CodegenError(format!("ELF function `{name}` has no section")))?;
            let section = file.section_by_index(section_index).map_err(error)?;
            let flags = section.flags();
            if !matches!(flags, object::SectionFlags::Elf { sh_flags } if sh_flags & u64::from(elf::SHF_EXECINSTR) != 0)
            {
                return Err(CodegenError(format!(
                    "ELF function `{name}` is outside executable code"
                )));
            }
            let start = usize::try_from(symbol.address()).map_err(error)?;
            let size = usize::try_from(symbol.size()).map_err(error)?;
            let end = start
                .checked_add(size)
                .ok_or_else(|| CodegenError("ELF function extent overflows".into()))?;
            let bytes = section
                .data()
                .map_err(error)?
                .get(start..end)
                .filter(|bytes| !bytes.is_empty())
                .ok_or_else(|| {
                    CodegenError(format!(
                        "ELF function `{name}` has an empty or out-of-range extent"
                    ))
                })?
                .to_vec();
            if by_address
                .insert((section_index.0, symbol.address()), name.clone())
                .is_some()
                || functions
                    .insert(
                        name.clone(),
                        ElfFunction {
                            symbol: name.clone(),
                            section: section_index,
                            text: TextSection {
                                address: symbol.address(),
                                bytes,
                            },
                        },
                    )
                    .is_some()
            {
                return Err(CodegenError(format!(
                    "ELF object repeats function name/address `{name}`"
                )));
            }
        }
        Ok(Self {
            functions,
            by_address,
        })
    }

    pub(super) fn function_at(
        &self,
        section: SectionIndex,
        offset: u64,
    ) -> Result<&ElfFunction, CodegenError> {
        self.by_address
            .get(&(section.0, offset))
            .and_then(|name| self.functions.get(name))
            .ok_or_else(|| {
                CodegenError(format!(
                    "ELF section {} offset {offset:#x} is not a function start",
                    section.0
                ))
            })
    }
}

pub(crate) fn verify_elf_artifact(
    path: &Path,
    expected_safepoints: &ExpectedSafepoints,
    expected_eh: &ExpectedEh,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    if file.format() != BinaryFormat::Elf
        || !file.is_64()
        || !file.is_little_endian()
        || file.kind() != ObjectKind::Relocatable
        || file.architecture() != Architecture::X86_64
    {
        return Err(CodegenError(
            "Linux amd64 codegen requires ELF64LE/x86_64 ET_REL".into(),
        ));
    }
    let code = ElfCode::read(&file)?;
    let mut stackmap = None;
    for section in file.sections() {
        if section.name().map_err(error)? == ".llvm_stackmaps"
            && stackmap.replace(section).is_some()
        {
            return Err(CodegenError("ELF object repeats .llvm_stackmaps".into()));
        }
    }
    let observed = if let Some(section) = stackmap {
        let mut relocations = BTreeMap::new();
        for (offset, relocation) in section.relocations() {
            if relocation.flags()
                != (RelocationFlags::Elf {
                    r_type: elf::R_X86_64_64,
                })
                || relocation.addend() != 0
                || relocation.has_implicit_addend()
            {
                return Err(CodegenError(
                    "ELF stackmap requires absolute function-symbol RELA with zero addend".into(),
                ));
            }
            let RelocationTarget::Symbol(index) = relocation.target() else {
                return Err(CodegenError(
                    "ELF stackmap lacks a function symbol relocation".into(),
                ));
            };
            let symbol = file.symbol_by_index(index).map_err(error)?;
            let name = symbol.name().map_err(error)?;
            let function = code
                .functions
                .get(name)
                .filter(|function| {
                    symbol.kind() == SymbolKind::Text
                        && symbol.section_index() == Some(function.section)
                })
                .ok_or_else(|| {
                    CodegenError(format!(
                        "ELF stackmap target `{name}` is not a canonical function"
                    ))
                })?;
            if relocations
                .insert(
                    offset,
                    FunctionRelocation {
                        symbol: name.to_owned(),
                        address: function.text.address,
                    },
                )
                .is_some()
            {
                return Err(CodegenError(
                    "ELF stackmap repeats a function relocation".into(),
                ));
            }
        }
        let texts = code
            .functions
            .iter()
            .map(|(name, function)| (name.clone(), function.text.clone()))
            .collect();
        parse_stackmaps(
            section.data().map_err(error)?,
            &relocations,
            &texts,
            expected_safepoints,
            profile.stack_map_version(),
            profile.architecture(),
        )?
    } else if expected_safepoints.site_count() == 0 {
        ObservedSafepoints::default()
    } else {
        return Err(CodegenError(
            "ELF object has no .llvm_stackmaps for managed LIR".into(),
        ));
    };
    eh::verify_elf_eh(&file, &code, expected_eh, &observed, profile)
}

pub(super) fn error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("ELF artifact: {error}"))
}
