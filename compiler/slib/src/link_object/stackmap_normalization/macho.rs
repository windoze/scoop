//! Mach-O section extraction and function-address relocation qualification.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use scoop_wire::sha256;

use super::{
    LlvmStackmapSectionParseError, ParsedLlvmStackmapFunctionV3, parse_llvm_stackmap_section_v3,
};
use crate::link_object::{
    BuiltinLinkObjectSectionProfileV1, BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1,
    DarwinArm64RelocationTargetV1, DarwinArm64SymbolKindV1,
    ValidatedBuiltinObjectSectionInventoryV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedDarwinArm64StackmapFunctionV3 {
    parsed: ParsedLlvmStackmapFunctionV3,
    target_symbol_table_index: u32,
}

impl VerifiedDarwinArm64StackmapFunctionV3 {
    pub const fn parsed(&self) -> &ParsedLlvmStackmapFunctionV3 {
        &self.parsed
    }

    pub const fn target_symbol_table_index(&self) -> u32 {
        self.target_symbol_table_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedDarwinArm64StackmapSectionV3 {
    section_ordinal: NonZeroU32,
    constants: Vec<u64>,
    functions: Vec<VerifiedDarwinArm64StackmapFunctionV3>,
}

impl VerifiedDarwinArm64StackmapSectionV3 {
    pub const fn section_ordinal(&self) -> NonZeroU32 {
        self.section_ordinal
    }

    pub fn constants(&self) -> &[u64] {
        &self.constants
    }

    pub fn functions(&self) -> &[VerifiedDarwinArm64StackmapFunctionV3] {
        &self.functions
    }

    pub fn record_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.parsed.records().len())
            .sum()
    }
}

pub fn verify_darwin_arm64_stackmap_section_v3(
    object_bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
) -> Result<Option<VerifiedDarwinArm64StackmapSectionV3>, DarwinArm64StackmapSectionError> {
    let envelope = sections.envelope();
    if u64::try_from(object_bytes.len()).ok() != Some(envelope.byte_length())
        || sha256(object_bytes) != envelope.content_digest()
    {
        return Err(DarwinArm64StackmapSectionError::ObjectBytesMismatch);
    }
    if sections.profile() != BuiltinLinkObjectSectionProfileV1::ScoopLir {
        return Err(DarwinArm64StackmapSectionError::WrongSectionProfile(
            sections.profile(),
        ));
    }
    let Some(section_index) = sections
        .roles()
        .iter()
        .position(|role| *role == BuiltinObjectSectionRoleV1::LlvmStackmaps)
    else {
        return Ok(None);
    };
    let section_ordinal = u32::try_from(section_index + 1)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or(DarwinArm64StackmapSectionError::SectionOrdinalOverflow)?;
    let section = envelope.sections()[section_index];
    let file_start = section
        .file_offset()
        .ok_or(DarwinArm64StackmapSectionError::SectionNotFileBacked)?;
    let file_end = file_start
        .checked_add(section.byte_size())
        .ok_or(DarwinArm64StackmapSectionError::SectionRangeOverflow)?;
    let start = usize::try_from(file_start)
        .map_err(|_| DarwinArm64StackmapSectionError::SectionRangeOverflow)?;
    let end = usize::try_from(file_end)
        .map_err(|_| DarwinArm64StackmapSectionError::SectionRangeOverflow)?;
    let contents = object_bytes
        .get(start..end)
        .ok_or(DarwinArm64StackmapSectionError::SectionRangeOverflow)?;
    let parsed =
        parse_llvm_stackmap_section_v3(contents).map_err(DarwinArm64StackmapSectionError::Parse)?;
    let (constants, parsed_functions) = parsed.into_parts();

    let mut relocations = envelope
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_section_ordinal() == section_ordinal)
        .map(|relocation| (relocation.offset(), relocation))
        .collect::<BTreeMap<_, _>>();
    let mut functions = Vec::with_capacity(parsed_functions.len());
    for (index, parsed) in parsed_functions.into_iter().enumerate() {
        if parsed.encoded_function_address() != 0 {
            return Err(
                DarwinArm64StackmapSectionError::PreRelocatedFunctionAddress {
                    index,
                    value: parsed.encoded_function_address(),
                },
            );
        }
        let offset = u32::try_from(parsed.function_address_offset()).map_err(|_| {
            DarwinArm64StackmapSectionError::FunctionAddressOffsetOverflow {
                index,
                offset: parsed.function_address_offset(),
            }
        })?;
        let relocation = relocations.remove(&offset).ok_or(
            DarwinArm64StackmapSectionError::MissingFunctionAddressRelocation { index, offset },
        )?;
        let DarwinArm64RelocationShapeV1::Unsigned64 {
            target: DarwinArm64RelocationTargetV1::SymbolTableIndex(target_symbol_table_index),
        } = relocation.shape()
        else {
            return Err(
                DarwinArm64StackmapSectionError::InvalidFunctionAddressRelocation {
                    index,
                    shape: relocation.shape(),
                },
            );
        };
        let symbol = envelope
            .symbols()
            .get(target_symbol_table_index as usize)
            .ok_or(
                DarwinArm64StackmapSectionError::InvalidFunctionTargetSymbol {
                    index,
                    table_index: target_symbol_table_index,
                },
            )?;
        if !matches!(
            symbol.kind(),
            DarwinArm64SymbolKindV1::ExternalStrongDefinition
                | DarwinArm64SymbolKindV1::ExternalWeakDefinition
        ) {
            return Err(
                DarwinArm64StackmapSectionError::InvalidFunctionTargetSymbol {
                    index,
                    table_index: target_symbol_table_index,
                },
            );
        }
        functions.push(VerifiedDarwinArm64StackmapFunctionV3 {
            parsed,
            target_symbol_table_index,
        });
    }
    if let Some((offset, _)) = relocations.first_key_value() {
        return Err(DarwinArm64StackmapSectionError::UnexpectedRelocation { offset: *offset });
    }
    Ok(Some(VerifiedDarwinArm64StackmapSectionV3 {
        section_ordinal,
        constants,
        functions,
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DarwinArm64StackmapSectionError {
    ObjectBytesMismatch,
    WrongSectionProfile(BuiltinLinkObjectSectionProfileV1),
    SectionOrdinalOverflow,
    SectionNotFileBacked,
    SectionRangeOverflow,
    Parse(LlvmStackmapSectionParseError),
    FunctionAddressOffsetOverflow {
        index: usize,
        offset: u64,
    },
    PreRelocatedFunctionAddress {
        index: usize,
        value: u64,
    },
    MissingFunctionAddressRelocation {
        index: usize,
        offset: u32,
    },
    InvalidFunctionAddressRelocation {
        index: usize,
        shape: DarwinArm64RelocationShapeV1,
    },
    InvalidFunctionTargetSymbol {
        index: usize,
        table_index: u32,
    },
    UnexpectedRelocation {
        offset: u32,
    },
}

impl fmt::Display for DarwinArm64StackmapSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin/AArch64 LLVM stackmap section: {self:?}"
        )
    }
}

impl std::error::Error for DarwinArm64StackmapSectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
