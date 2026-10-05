//! Stackmap section extraction and format-specific function-address relocations.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use crate::link_object::ObjectRelocationShapeV1;
use scoop_wire::sha256;

use super::{
    LlvmStackmapSectionParseError, ParsedLlvmStackmapFunctionV3, parse_llvm_stackmap_section_v3,
};
use crate::link_object::{
    BuiltinLinkObjectSectionProfileV1, BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1,
    DarwinArm64RelocationTargetV1, ObjectSymbolKindV1, ValidatedBuiltinObjectSectionInventoryV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedObjectStackmapFunctionV3 {
    parsed: ParsedLlvmStackmapFunctionV3,
    target_symbol_table_index: u32,
}

impl VerifiedObjectStackmapFunctionV3 {
    pub const fn parsed(&self) -> &ParsedLlvmStackmapFunctionV3 {
        &self.parsed
    }

    pub const fn target_symbol_table_index(&self) -> u32 {
        self.target_symbol_table_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedObjectStackmapSectionV3 {
    section_ordinal: NonZeroU32,
    constants: Vec<u64>,
    functions: Vec<VerifiedObjectStackmapFunctionV3>,
}

impl VerifiedObjectStackmapSectionV3 {
    pub const fn section_ordinal(&self) -> NonZeroU32 {
        self.section_ordinal
    }

    pub fn constants(&self) -> &[u64] {
        &self.constants
    }

    pub fn functions(&self) -> &[VerifiedObjectStackmapFunctionV3] {
        &self.functions
    }

    pub fn record_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.parsed.records().len())
            .sum()
    }
}

pub fn verify_object_stackmap_section_v3(
    object_bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
) -> Result<Option<VerifiedObjectStackmapSectionV3>, ObjectStackmapSectionError> {
    let envelope = sections.envelope();
    if u64::try_from(object_bytes.len()).ok() != Some(envelope.byte_length())
        || sha256(object_bytes) != envelope.content_digest()
    {
        return Err(ObjectStackmapSectionError::ObjectBytesMismatch);
    }
    if sections.profile() != BuiltinLinkObjectSectionProfileV1::ScoopLir {
        return Err(ObjectStackmapSectionError::WrongSectionProfile(
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
        .ok_or(ObjectStackmapSectionError::SectionOrdinalOverflow)?;
    let section = &envelope.sections()[section_index];
    let file_start = section
        .file_offset()
        .ok_or(ObjectStackmapSectionError::SectionNotFileBacked)?;
    let file_end = file_start
        .checked_add(section.byte_size())
        .ok_or(ObjectStackmapSectionError::SectionRangeOverflow)?;
    let start = usize::try_from(file_start)
        .map_err(|_| ObjectStackmapSectionError::SectionRangeOverflow)?;
    let end =
        usize::try_from(file_end).map_err(|_| ObjectStackmapSectionError::SectionRangeOverflow)?;
    let contents = object_bytes
        .get(start..end)
        .ok_or(ObjectStackmapSectionError::SectionRangeOverflow)?;
    let parsed =
        parse_llvm_stackmap_section_v3(contents).map_err(ObjectStackmapSectionError::Parse)?;
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
            return Err(ObjectStackmapSectionError::PreRelocatedFunctionAddress {
                index,
                value: parsed.encoded_function_address(),
            });
        }
        let offset = u32::try_from(parsed.function_address_offset()).map_err(|_| {
            ObjectStackmapSectionError::FunctionAddressOffsetOverflow {
                index,
                offset: parsed.function_address_offset(),
            }
        })?;
        let relocation = relocations.remove(&u64::from(offset)).ok_or(
            ObjectStackmapSectionError::MissingFunctionAddressRelocation { index, offset },
        )?;
        let target_symbol_table_index = match relocation.shape() {
            ObjectRelocationShapeV1::DarwinArm64(DarwinArm64RelocationShapeV1::Unsigned64 {
                target: DarwinArm64RelocationTargetV1::SymbolTableIndex(target),
            }) => target,
            ObjectRelocationShapeV1::ElfRela {
                kind: object::elf::R_X86_64_64,
                target_symbol,
                addend: 0,
                width: 8,
            } => target_symbol,
            shape => {
                return Err(
                    ObjectStackmapSectionError::InvalidFunctionAddressRelocation { index, shape },
                );
            }
        };
        let symbol = envelope
            .symbols()
            .get(target_symbol_table_index as usize)
            .ok_or(ObjectStackmapSectionError::InvalidFunctionTargetSymbol {
                index,
                table_index: target_symbol_table_index,
            })?;
        if !matches!(
            symbol.kind(),
            ObjectSymbolKindV1::ExternalStrongDefinition
                | ObjectSymbolKindV1::ExternalWeakDefinition
        ) {
            return Err(ObjectStackmapSectionError::InvalidFunctionTargetSymbol {
                index,
                table_index: target_symbol_table_index,
            });
        }
        functions.push(VerifiedObjectStackmapFunctionV3 {
            parsed,
            target_symbol_table_index,
        });
    }
    if let Some((offset, _)) = relocations.first_key_value() {
        return Err(ObjectStackmapSectionError::UnexpectedRelocation { offset: *offset });
    }
    Ok(Some(VerifiedObjectStackmapSectionV3 {
        section_ordinal,
        constants,
        functions,
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectStackmapSectionError {
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
        shape: ObjectRelocationShapeV1,
    },
    InvalidFunctionTargetSymbol {
        index: usize,
        table_index: u32,
    },
    UnexpectedRelocation {
        offset: u64,
    },
}

impl fmt::Display for ObjectStackmapSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid LLVM stackmap object section: {self:?}")
    }
}

impl std::error::Error for ObjectStackmapSectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
