use super::*;
use object::elf;
use object::read::elf::{Rela, SectionHeader};

pub(super) fn read(file: &ElfFile64<'_>) -> Result<Vec<ElfRelocation>, ElfObjectError> {
    let endian = file.endian();
    let mut result = Vec::new();
    for section in file.elf_section_table().iter() {
        if matches!(
            section.sh_type(endian),
            elf::SHT_REL | elf::SHT_RELR | elf::SHT_CREL
        ) {
            return Err(error("this ELF target requires RELA relocation tables"));
        }
        let Some((relocations, symbols)) = section.rela(endian, file.data()).map_err(error)? else {
            continue;
        };
        if section.sh_entsize(endian) != 24 || symbols != file.elf_symbol_table().section() {
            return Err(error("invalid RELA entry size or symbol table"));
        }
        let target = SectionIndex(section.sh_info(endian) as usize);
        let target_section = file.elf_section_table().section(target).map_err(error)?;
        if target.0 == 0 || target_section.sh_type(endian) == elf::SHT_NOBITS {
            return Err(error("RELA target has no file-backed storage"));
        }
        for relocation in relocations {
            let kind = relocation.r_type(endian, false);
            let width = match file.architecture() {
                Architecture::X86_64 => x86_64::relocation_width(kind)?,
                _ => return Err(error("unsupported ELF relocation machine")),
            };
            let offset = relocation.r_offset(endian);
            if offset
                .checked_add(u64::from(width))
                .is_none_or(|end| end > target_section.sh_size(endian))
            {
                return Err(error("relocation write extends past its section"));
            }
            let symbol = SymbolIndex(relocation.r_sym(endian, false) as usize);
            // The null symbol denotes the absolute zero base in ELF RELA.
            if symbol.0 >= file.elf_symbol_table().len() {
                return Err(error("relocation references a missing symbol"));
            }
            result.push(ElfRelocation {
                section: target,
                offset,
                symbol,
                addend: relocation.r_addend(endian),
                kind,
                width,
            });
        }
    }
    result.sort_by_key(|relocation| (relocation.section.0, relocation.offset));
    Ok(result)
}
