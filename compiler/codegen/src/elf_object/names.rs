//! Rename backend contributions while keeping their relocation indices intact.

use super::*;

impl ElfObject<'_> {
    pub(crate) fn rename_symbol(
        &mut self,
        previous: &str,
        name: &str,
        linkage: LinkageClass,
    ) -> Result<(), CodegenError> {
        if name.is_empty()
            || name.as_bytes().contains(&0)
            || self.file.symbol_by_name(name).is_some()
            || !self.names.insert(name.to_owned())
        {
            return Err(CodegenError(format!(
                "invalid or repeated ELF symbol `{name}`"
            )));
        }
        let symbol = self
            .file
            .symbol_by_name(previous)
            .ok_or_else(|| CodegenError(format!("missing ELF symbol `{previous}`")))?;
        if symbol.is_local() || symbol.section_index().is_none() {
            return Err(CodegenError(
                "renamed ELF contribution must be an external definition".into(),
            ));
        }
        let name_offset = u32::try_from(self.strings.len()).map_err(error)?;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        let offset = symbol
            .index()
            .0
            .checked_mul(24)
            .ok_or_else(|| error("symbol offset overflows"))?;
        let entry = self
            .symbols
            .get_mut(offset..offset + 24)
            .ok_or_else(|| error("symbol is outside its table"))?;
        entry[..4].copy_from_slice(&name_offset.to_le_bytes());
        entry[4] = binding(linkage)? << 4 | (entry[4] & 15);
        entry[5] = (entry[5] & !3) | elf::STV_HIDDEN;
        Ok(())
    }

    pub(crate) fn rename_section(
        &mut self,
        section: SectionIndex,
        name: &str,
    ) -> Result<(), CodegenError> {
        if name.is_empty() || name.as_bytes().contains(&0) {
            return Err(error("invalid ELF section name"));
        }
        let index = self
            .file
            .elf_header()
            .section_strings_index(LE, self.file.data())
            .map_err(error)?;
        // LLVM may share the section-name and symbol-name string tables.
        let strings = if index == self.strtab {
            &mut self.strings
        } else {
            if self.section_strings.is_none() {
                let bytes = self
                    .file
                    .section_by_index(index)
                    .map_err(error)?
                    .data()
                    .map_err(error)?
                    .to_vec();
                self.section_strings = Some((index, bytes));
            }
            &mut self
                .section_strings
                .as_mut()
                .expect("section string table initialized")
                .1
        };
        let offset = u32::try_from(strings.len()).map_err(error)?;
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
        self.write_u32(self.header_offset(section)?, offset)
    }
}
