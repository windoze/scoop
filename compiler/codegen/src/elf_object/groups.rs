//! Join LLVM backend contributions to their callable's existing ELF group.

use super::*;

impl ElfObject<'_> {
    pub(crate) fn associate_sections(
        &mut self,
        signature: &str,
        sections: &[SectionIndex],
    ) -> Result<(), CodegenError> {
        let symbol = self
            .file
            .symbol_by_name(signature)
            .ok_or_else(|| CodegenError(format!("missing ELF COMDAT signature `{signature}`")))?;
        let group = self
            .file
            .sections()
            .find(|section| {
                let header = section.elf_section_header();
                header.sh_type(LE) == elf::SHT_GROUP
                    && header.sh_info(LE) as usize == symbol.index().0
            })
            .ok_or_else(|| {
                CodegenError(format!(
                    "ELF ODR definition `{signature}` has no COMDAT group"
                ))
            })?;
        let group_index = group.index();
        let data = group.data().map_err(error)?;
        if data.len() < 4 || data.len() % 4 != 0 || data[..4] != elf::GRP_COMDAT.to_le_bytes() {
            return Err(CodegenError("ELF ODR section group is malformed".into()));
        }
        let mut members = data[4..]
            .chunks_exact(4)
            .map(|entry| u32::from_le_bytes(entry.try_into().expect("group entry")) as usize)
            .collect::<BTreeSet<_>>();
        let mut additions = sections
            .iter()
            .map(|section| section.0)
            .collect::<BTreeSet<_>>();
        for section in self.file.sections() {
            let header = section.elf_section_header();
            if header.sh_type(LE) == elf::SHT_RELA
                && additions.contains(&(header.sh_info(LE) as usize))
            {
                additions.insert(section.index().0);
            }
        }
        for index in &additions {
            if members.contains(index) {
                continue;
            }
            let section = self
                .file
                .section_by_index(SectionIndex(*index))
                .map_err(error)?;
            if section.elf_section_header().sh_flags(LE) & u64::from(elf::SHF_GROUP) != 0 {
                return Err(CodegenError(
                    "ELF associated atom already belongs to another group".into(),
                ));
            }
        }
        for index in &additions {
            self.add_section_flags(SectionIndex(*index), u64::from(elf::SHF_GROUP))?;
        }
        members.extend(additions);
        let mut updated = elf::GRP_COMDAT.to_le_bytes().to_vec();
        for index in members {
            updated.extend_from_slice(&u32::try_from(index).map_err(error)?.to_le_bytes());
        }
        self.replace_section(group_index, &updated)
    }
}
