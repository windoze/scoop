//! Append COFF symbols and update COMDAT association without renumbering inputs.

use std::collections::{BTreeMap, BTreeSet};

use object::{Object, ObjectSection, ObjectSymbol, SectionIndex, SymbolIndex, pe};
use scoop_lir::LinkageClass;

use crate::CodegenError;

mod constants;

pub(crate) use constants::merge_callable_constants;

pub(crate) struct CoffObject<'a> {
    pub(crate) file: object::File<'a>,
    bytes: Vec<u8>,
    symbols: Vec<u8>,
    strings: Vec<u8>,
    symbol_bytes: usize,
    table_offset_field: usize,
    table_count_field: usize,
    sections_offset: usize,
    original_symbols: BTreeMap<&'a [u8], SymbolIndex>,
    added: BTreeSet<String>,
    section_aux: BTreeMap<usize, usize>,
}

impl<'a> CoffObject<'a> {
    pub(crate) fn read(bytes: &'a [u8]) -> Result<Self, CodegenError> {
        let (symbol_bytes, table_offset_field, table_count_field, sections_offset) =
            match object::FileKind::parse(bytes).map_err(error)? {
                object::FileKind::Coff => (18, 8, 12, 20),
                object::FileKind::CoffBig => (20, 48, 52, 56),
                _ => return Err(error("atom materialization requires COFF or bigobj")),
            };
        let file = object::File::parse(bytes).map_err(error)?;
        if file.architecture() != object::Architecture::X86_64 {
            return Err(error("atom materialization requires AMD64"));
        }
        let start = field(bytes, table_offset_field)? as usize;
        let count = field(bytes, table_count_field)? as usize;
        let end = start
            .checked_add(
                count
                    .checked_mul(symbol_bytes)
                    .ok_or_else(|| error("symbol count overflows"))?,
            )
            .ok_or_else(|| error("symbol extent overflows"))?;
        let symbols = bytes
            .get(start..end)
            .ok_or_else(|| error("truncated symbol table"))?
            .to_vec();
        let string_size = field(bytes, end)? as usize;
        if string_size < 4 {
            return Err(error("invalid string table length"));
        }
        let strings = bytes
            .get(
                end..end
                    .checked_add(string_size)
                    .ok_or_else(|| error("string extent overflows"))?,
            )
            .ok_or_else(|| error("truncated string table"))?
            .to_vec();
        let mut original_symbols = BTreeMap::new();
        let mut section_aux = BTreeMap::new();
        for symbol in file.symbols() {
            let name = symbol.name_bytes().map_err(error)?;
            original_symbols.entry(name).or_insert(symbol.index());
            let offset = symbol.index().0 * symbol_bytes;
            if symbols[offset + symbol_bytes - 2] == pe::IMAGE_SYM_CLASS_STATIC
                && symbols[offset + symbol_bytes - 1] != 0
                && symbol.address() == 0
                && let Some(section) = symbol.section_index()
                && file
                    .section_by_index(section)
                    .map_err(error)?
                    .name_bytes()
                    .map_err(error)?
                    == name
            {
                section_aux.insert(section.0, offset + symbol_bytes);
            }
        }
        Ok(Self {
            file,
            bytes: bytes.to_vec(),
            symbols,
            strings,
            symbol_bytes,
            table_offset_field,
            table_count_field,
            sections_offset,
            original_symbols,
            added: BTreeSet::new(),
            section_aux,
        })
    }

    pub(crate) fn extent(&self, name: &str) -> Result<(SectionIndex, u64), CodegenError> {
        let index = self
            .original_symbols
            .get(name.as_bytes())
            .ok_or_else(|| error(format!("missing symbol `{name}`")))?;
        let symbol = self.file.symbol_by_index(*index).map_err(error)?;
        let section = symbol
            .section_index()
            .ok_or_else(|| error(format!("undefined symbol `{name}`")))?;
        Ok((section, symbol.address()))
    }

    pub(crate) fn boundary(
        &mut self,
        name: &str,
        section: SectionIndex,
        value: u64,
    ) -> Result<(), CodegenError> {
        if name.is_empty()
            || name.as_bytes().contains(&0)
            || self.original_symbols.contains_key(name.as_bytes())
            || !self.added.insert(name.to_owned())
        {
            return Err(error(format!("invalid or duplicate boundary `{name}`")));
        }
        if value > self.file.section_by_index(section).map_err(error)?.size() {
            return Err(error(format!("boundary `{name}` exceeds its section")));
        }
        let name_offset = u32::try_from(self.strings.len()).map_err(error)?;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        self.symbols.extend_from_slice(&[0; 4]);
        self.symbols.extend_from_slice(&name_offset.to_le_bytes());
        self.symbols
            .extend_from_slice(&u32::try_from(value).map_err(error)?.to_le_bytes());
        if self.symbol_bytes == 18 {
            self.symbols
                .extend_from_slice(&i16::try_from(section.0).map_err(error)?.to_le_bytes());
        } else {
            self.symbols
                .extend_from_slice(&i32::try_from(section.0).map_err(error)?.to_le_bytes());
        }
        self.symbols
            .extend_from_slice(&[0, 0, pe::IMAGE_SYM_CLASS_EXTERNAL, 0]);
        Ok(())
    }

    pub(crate) fn existing_pair(
        &self,
        boundary: scoop_lir::AtomBoundarySymbolsV1,
    ) -> Result<SectionIndex, CodegenError> {
        let (section, start) = self.extent(boundary.start().symbol().as_str())?;
        let (end_section, end) = self.extent(boundary.end().symbol().as_str())?;
        if section != end_section
            || start > end
            || end > self.file.section_by_index(section).map_err(error)?.size()
        {
            return Err(error("invalid existing atom boundary pair"));
        }
        Ok(section)
    }

    pub(crate) fn associate(
        &mut self,
        primary: SectionIndex,
        sections: &[SectionIndex],
        linkage: LinkageClass,
    ) -> Result<(), CodegenError> {
        let selection = match linkage {
            LinkageClass::ConeStrong | LinkageClass::TemplateSupportHidden => {
                pe::IMAGE_COMDAT_SELECT_NODUPLICATES
            }
            LinkageClass::OdrWeak => pe::IMAGE_COMDAT_SELECT_ANY,
            LinkageClass::RuntimeAbi => {
                return Err(error("runtime ABI symbol cannot own a COFF atom"));
            }
        };
        for section in sections
            .iter()
            .copied()
            .chain(std::iter::once(primary))
            .map(|section| section.0)
            .collect::<BTreeSet<_>>()
        {
            let aux = *self
                .section_aux
                .get(&section)
                .ok_or_else(|| error("section definition has no auxiliary record"))?;
            let section = SectionIndex(section);
            let characteristic_offset = self.sections_offset + (section.0 - 1) * 40 + 36;
            let flags = field(&self.bytes, characteristic_offset)? | pe::IMAGE_SCN_LNK_COMDAT;
            self.bytes[characteristic_offset..characteristic_offset + 4]
                .copy_from_slice(&flags.to_le_bytes());
            let association = if section == primary {
                0
            } else {
                u32::try_from(primary.0).map_err(error)?
            };
            self.symbols[aux + 12..aux + 14].copy_from_slice(&(association as u16).to_le_bytes());
            self.symbols[aux + 14] = if section == primary {
                selection
            } else {
                pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE
            };
            self.symbols[aux + 16..aux + 18]
                .copy_from_slice(&((association >> 16) as u16).to_le_bytes());
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<Vec<u8>, CodegenError> {
        let offset = u32::try_from(self.bytes.len()).map_err(error)?;
        let count = u32::try_from(self.symbols.len() / self.symbol_bytes).map_err(error)?;
        let length = u32::try_from(self.strings.len()).map_err(error)?;
        self.strings[..4].copy_from_slice(&length.to_le_bytes());
        self.bytes[self.table_offset_field..self.table_offset_field + 4]
            .copy_from_slice(&offset.to_le_bytes());
        self.bytes[self.table_count_field..self.table_count_field + 4]
            .copy_from_slice(&count.to_le_bytes());
        self.bytes.extend(self.symbols);
        self.bytes.extend(self.strings);
        Ok(self.bytes)
    }
}

fn field(bytes: &[u8], offset: usize) -> Result<u32, CodegenError> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| error("truncated header"))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte field"),
    ))
}

pub(crate) fn error(message: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("COFF object: {message}"))
}
