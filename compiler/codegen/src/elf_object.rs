//! Small ELF edits that preserve all original section and symbol indices.
//!
//! The reader is `object`. Only appended symbol/string payloads and existing
//! header fields are written here, so RELA, COMDAT, merge-entry sizes, and TLS
//! details do not need to be reconstructed through a second object writer.

use std::collections::BTreeSet;

use object::read::elf::{ElfFile64, FileHeader, SectionHeader};
use object::{LittleEndian as LE, Object, ObjectSection, ObjectSymbol, SectionIndex, elf};
use scoop_lir::LinkageClass;

use crate::CodegenError;

mod groups;
mod names;

pub(crate) struct ElfObject<'a> {
    pub(crate) file: ElfFile64<'a, LE>,
    bytes: Vec<u8>,
    symbols: Vec<u8>,
    strings: Vec<u8>,
    symtab: SectionIndex,
    strtab: SectionIndex,
    names: BTreeSet<String>,
    section_strings: Option<(SectionIndex, Vec<u8>)>,
}

impl<'a> ElfObject<'a> {
    pub(crate) fn read(bytes: &'a [u8]) -> Result<Self, CodegenError> {
        let file = ElfFile64::<LE>::parse(bytes).map_err(error)?;
        if !file.is_little_endian()
            || file.elf_header().e_type(LE) != elf::ET_REL
            || file.elf_header().e_shentsize(LE) != 64
        {
            return Err(CodegenError(
                "ELF atom materialization requires ELF64LE ET_REL".into(),
            ));
        }
        let tables = file
            .sections()
            .filter(|section| section.elf_section_header().sh_type(LE) == elf::SHT_SYMTAB)
            .collect::<Vec<_>>();
        let [symtab] = tables.as_slice() else {
            return Err(CodegenError(
                "ELF atom materialization requires one symbol table".into(),
            ));
        };
        if symtab.elf_section_header().sh_entsize(LE) != 24 {
            return Err(CodegenError(
                "ELF symbol table has an invalid entry size".into(),
            ));
        }
        let strtab = SectionIndex(symtab.elf_section_header().sh_link(LE) as usize);
        let strings = file
            .section_by_index(strtab)
            .map_err(error)?
            .data()
            .map_err(error)?
            .to_vec();
        let symbols = symtab.data().map_err(error)?.to_vec();
        let symtab = symtab.index();
        Ok(Self {
            file,
            bytes: bytes.to_vec(),
            symbols,
            strings,
            symtab,
            strtab,
            names: BTreeSet::new(),
            section_strings: None,
        })
    }

    pub(crate) fn boundary(
        &mut self,
        name: &str,
        section: SectionIndex,
        value: u64,
        linkage: LinkageClass,
    ) -> Result<(), CodegenError> {
        if name.as_bytes().contains(&0)
            || name.is_empty()
            || self.file.symbol_by_name(name).is_some()
            || !self.names.insert(name.to_owned())
        {
            return Err(CodegenError(format!(
                "ELF boundary `{name}` is invalid or already defined"
            )));
        }
        let target = self.file.section_by_index(section).map_err(error)?;
        if value > target.size() {
            return Err(CodegenError(format!(
                "ELF boundary `{name}` escapes its section"
            )));
        }
        let index = u16::try_from(section.0).map_err(error)?;
        if index >= elf::SHN_LORESERVE {
            return Err(CodegenError(
                "ELF callable boundary section requires an extended symbol index".into(),
            ));
        }
        let name_offset = u32::try_from(self.strings.len()).map_err(error)?;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        self.symbols.extend_from_slice(&name_offset.to_le_bytes());
        self.symbols.push(binding(linkage)? << 4 | elf::STT_OBJECT);
        self.symbols.push(elf::STV_HIDDEN);
        self.symbols.extend_from_slice(&index.to_le_bytes());
        self.symbols.extend_from_slice(&value.to_le_bytes());
        self.symbols.extend_from_slice(&0u64.to_le_bytes());
        Ok(())
    }

    pub(crate) fn existing_boundary(
        &mut self,
        name: &str,
        linkage: LinkageClass,
        end: bool,
    ) -> Result<(), CodegenError> {
        let symbol = self
            .file
            .symbol_by_name(name)
            .ok_or_else(|| CodegenError(format!("ELF atom boundary `{name}` is missing")))?;
        if symbol.section_index().is_none() || symbol.is_local() {
            return Err(CodegenError(format!(
                "ELF atom boundary `{name}` is not an external section definition"
            )));
        }
        let offset = symbol
            .index()
            .0
            .checked_mul(24)
            .ok_or_else(|| CodegenError("ELF symbol offset overflows".into()))?;
        let entry = self
            .symbols
            .get_mut(offset..offset + 24)
            .ok_or_else(|| CodegenError("ELF symbol lies outside its table".into()))?;
        entry[4] = binding(linkage)? << 4 | (entry[4] & 15);
        entry[5] = (entry[5] & !3) | elf::STV_HIDDEN;
        if end {
            entry[16..24].copy_from_slice(&0u64.to_le_bytes());
        }
        Ok(())
    }

    pub(crate) fn relocatable_metadata(&mut self) -> Result<(), CodegenError> {
        let sections = self
            .file
            .sections()
            .filter_map(|section| {
                section.name().ok().and_then(|name| {
                    (name == ".llvm_stackmaps" || name.starts_with(".data.rel.ro.scoop."))
                        .then_some(section.index())
                })
            })
            .collect::<Vec<_>>();
        for section in sections {
            self.add_section_flags(section, u64::from(elf::SHF_ALLOC | elf::SHF_WRITE))?;
        }
        Ok(())
    }

    fn add_section_flags(&mut self, section: SectionIndex, flags: u64) -> Result<(), CodegenError> {
        let field = self.header_offset(section)? + 8;
        self.write_u64(field, self.section_flags(section)? | flags)
    }

    fn section_flags(&self, section: SectionIndex) -> Result<u64, CodegenError> {
        let field = self.header_offset(section)? + 8;
        Ok(u64::from_le_bytes(
            self.bytes
                .get(field..field + 8)
                .ok_or_else(|| CodegenError("ELF section flags are out of range".into()))?
                .try_into()
                .expect("ELF64 field"),
        ))
    }

    fn header_offset(&self, section: SectionIndex) -> Result<usize, CodegenError> {
        self.file.section_by_index(section).map_err(error)?;
        usize::try_from(self.file.elf_header().e_shoff(LE))
            .ok()
            .and_then(|start| {
                section
                    .0
                    .checked_mul(64)
                    .and_then(|offset| start.checked_add(offset))
            })
            .ok_or_else(|| CodegenError("ELF section header offset overflows".into()))
    }

    fn write_u64(&mut self, field: usize, value: u64) -> Result<(), CodegenError> {
        self.bytes
            .get_mut(field..field + 8)
            .ok_or_else(|| CodegenError("ELF field is outside the file".into()))?
            .copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_u32(&mut self, field: usize, value: u32) -> Result<(), CodegenError> {
        self.bytes
            .get_mut(field..field + 4)
            .ok_or_else(|| CodegenError("ELF field is outside the file".into()))?
            .copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn replace_section(&mut self, section: SectionIndex, data: &[u8]) -> Result<(), CodegenError> {
        let old = self.file.section_by_index(section).map_err(error)?;
        let header = self.header_offset(section)?;
        let (old_offset, old_size) = old
            .file_range()
            .ok_or_else(|| CodegenError("ELF edited section has no file bytes".into()))?;
        if old_size == data.len() as u64 {
            let offset = usize::try_from(old_offset).map_err(error)?;
            self.bytes
                .get_mut(offset..offset + data.len())
                .ok_or_else(|| CodegenError("ELF section is outside the file".into()))?
                .copy_from_slice(data);
        } else {
            let alignment = usize::try_from(old.align().max(1)).map_err(error)?;
            let offset = self
                .bytes
                .len()
                .checked_next_multiple_of(alignment)
                .ok_or_else(|| CodegenError("ELF appended section offset overflows".into()))?;
            self.bytes.resize(offset, 0);
            self.bytes.extend_from_slice(data);
            self.write_u64(header + 24, offset as u64)?;
            self.write_u64(header + 32, data.len() as u64)?;
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<Vec<u8>, CodegenError> {
        let strings = std::mem::take(&mut self.strings);
        let symbols = std::mem::take(&mut self.symbols);
        self.replace_section(self.strtab, &strings)?;
        self.replace_section(self.symtab, &symbols)?;
        if let Some((index, strings)) = self.section_strings.take() {
            self.replace_section(index, &strings)?;
        }
        Ok(self.bytes)
    }
}

fn binding(linkage: LinkageClass) -> Result<u8, CodegenError> {
    match linkage {
        LinkageClass::ConeStrong | LinkageClass::TemplateSupportHidden => Ok(elf::STB_GLOBAL),
        LinkageClass::OdrWeak => Ok(elf::STB_WEAK),
        LinkageClass::RuntimeAbi => Err(CodegenError(
            "runtime ABI symbol cannot be an ELF atom boundary".into(),
        )),
    }
}

pub(super) fn error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("ELF atom materialization: {error}"))
}
