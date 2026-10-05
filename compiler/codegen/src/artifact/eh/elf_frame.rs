//! LLVM ELF CIE/FDE records; LSDA interpretation remains in the shared decoder.

use std::collections::{BTreeMap, BTreeSet};

use object::{Object, ObjectSection, SectionIndex};

use crate::CodegenError;
use crate::artifact::elf::error;
use crate::target::CodeArchitecture;

use super::cursor::Cursor;
use super::elf_relocations::{self, ElfPoint};

pub(super) struct ElfFde {
    pub(super) function: ElfPoint,
    pub(super) size: u64,
    pub(super) lsda: Option<ElfPoint>,
}

struct FrameReader<'data, 'file> {
    file: &'file object::File<'data>,
    architecture: CodeArchitecture,
    relocations: BTreeMap<u64, object::Relocation>,
}

pub(super) fn parse(
    file: &object::File<'_>,
    section: SectionIndex,
    architecture: CodeArchitecture,
) -> Result<Vec<ElfFde>, CodegenError> {
    let section = file.section_by_index(section).map_err(error)?;
    let mut reader = FrameReader {
        file,
        architecture,
        relocations: BTreeMap::new(),
    };
    for (offset, relocation) in section.relocations() {
        if reader.relocations.insert(offset, relocation).is_some() {
            return Err(CodegenError(
                "ELF .eh_frame repeats a relocation offset".into(),
            ));
        }
    }
    let bytes = section.data().map_err(error)?;
    let mut cursor = Cursor::new(bytes, "ELF .eh_frame");
    let mut cies = BTreeMap::new();
    let mut used_cies = BTreeSet::new();
    let mut result = Vec::new();
    while cursor.position() < bytes.len() {
        let record_start = cursor.position();
        if bytes[record_start..].iter().all(|byte| *byte == 0) {
            break;
        }
        let length = cursor.u32("record length")?;
        if length < 4 || length == u32::MAX {
            return Err(CodegenError(
                "ELF .eh_frame requires bounded DWARF32 records".into(),
            ));
        }
        let base = cursor.position();
        let body = cursor.take(length as usize, "record body")?;
        let mut record = Cursor::new(body, "ELF .eh_frame record");
        let cie_pointer = record.u32("CIE reference")?;
        if cie_pointer == 0 {
            let has_lsda = reader.cie(base, &mut record)?;
            cies.insert(record_start, has_lsda);
        } else {
            let cie_offset = base
                .checked_sub(cie_pointer as usize)
                .ok_or_else(|| CodegenError("ELF FDE CIE reference underflows".into()))?;
            let has_lsda = *cies
                .get(&cie_offset)
                .ok_or_else(|| CodegenError("ELF FDE references an unknown CIE".into()))?;
            used_cies.insert(cie_offset);
            let function = reader.pointer(base, &mut record)?;
            let size = u64::from(record.u32("FDE address range")?);
            let length = record.uleb("FDE augmentation length")?;
            let lsda = if has_lsda {
                if length != 4 {
                    return Err(CodegenError(
                        "ELF Scoop FDE requires one signed-32 LSDA pointer".into(),
                    ));
                }
                Some(reader.pointer(base, &mut record)?)
            } else {
                if length != 0 {
                    return Err(CodegenError(
                        "ELF unwind-only FDE has an unexpected augmentation".into(),
                    ));
                }
                None
            };
            result.push(ElfFde {
                function,
                size,
                lsda,
            });
        }
    }
    if !reader.relocations.is_empty() {
        return Err(CodegenError(
            "ELF .eh_frame has relocations outside the CIE/FDE pointer fields".into(),
        ));
    }
    if cies.len() != used_cies.len() {
        return Err(CodegenError("ELF .eh_frame has an unreferenced CIE".into()));
    }
    Ok(result)
}

impl FrameReader<'_, '_> {
    fn pointer(&mut self, base: usize, cursor: &mut Cursor<'_>) -> Result<ElfPoint, CodegenError> {
        let field = base
            .checked_add(cursor.position())
            .ok_or_else(|| CodegenError("ELF EH field offset overflows".into()))?;
        if cursor.u32("PC-relative signed-32 pointer")? != 0 {
            return Err(CodegenError(
                "ELF EH RELA pointer has a nonzero in-place addend".into(),
            ));
        }
        let relocation = self.relocations.remove(&(field as u64)).ok_or_else(|| {
            CodegenError(format!("ELF EH pointer at {field:#x} lacks a relocation"))
        })?;
        elf_relocations::resolve(self.file, &relocation, self.architecture)
    }

    fn cie(&mut self, base: usize, cursor: &mut Cursor<'_>) -> Result<bool, CodegenError> {
        if cursor.u8("CIE version")? != 1 {
            return Err(CodegenError("ELF CIE requires version 1".into()));
        }
        let augmentation = cursor.nul_terminated("CIE augmentation")?;
        let expected_register = match self.architecture {
            CodeArchitecture::Aarch64 => 30,
            CodeArchitecture::X86_64 => 16,
        };
        if cursor.uleb("CIE code alignment")? != 1
            || cursor.sleb("CIE data alignment")? != -8
            || cursor.u8("CIE return-address register")? != expected_register
        {
            return Err(CodegenError(
                "ELF CIE violates its architecture alignment/register contract".into(),
            ));
        }
        let length = usize::try_from(cursor.uleb("CIE augmentation length")?).map_err(error)?;
        let end = cursor
            .position()
            .checked_add(length)
            .ok_or_else(|| CodegenError("ELF CIE augmentation extent overflows".into()))?;
        let has_lsda = match augmentation {
            b"zPLR" => {
                if cursor.u8("personality encoding")? != 0x9b {
                    return Err(CodegenError(
                        "ELF CIE requires an indirect PC-relative signed-32 personality".into(),
                    ));
                }
                let personality = self.pointer(base, cursor)?;
                elf_relocations::validate_personality(self.file, personality, self.architecture)?;
                if cursor.u8("LSDA encoding")? != 0x1b {
                    return Err(CodegenError(
                        "ELF CIE requires PC-relative signed-32 LSDA encoding".into(),
                    ));
                }
                true
            }
            b"zR" => false,
            _ => {
                return Err(CodegenError(
                    "ELF CIE has an unsupported augmentation".into(),
                ));
            }
        };
        if cursor.u8("FDE encoding")? != 0x1b || cursor.position() != end {
            return Err(CodegenError("ELF CIE requires PC-relative signed-32 FDE encoding and an exact augmentation size".into()));
        }
        Ok(has_lsda)
    }
}
