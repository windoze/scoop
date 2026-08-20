use super::*;
use object::read::elf::SectionHeader;
use object::{ObjectSection, ObjectSymbol, SymbolKind, elf};
use std::collections::BTreeSet;

pub(super) fn check(file: &ElfFile64<'_>) -> Result<(), ElfObjectError> {
    let endian = file.endian();
    let table = file.elf_section_table();
    let mut grouped = BTreeSet::new();
    for (index, section) in table.enumerate().skip(1) {
        table.section_name(endian, section).map_err(error)?;
        section.data(endian, file.data()).map_err(error)?;
        let alignment = section.sh_addralign(endian);
        if alignment != 0 && !alignment.is_power_of_two() {
            return Err(error("section alignment is not a power of two"));
        }
        if section.sh_type(endian) == elf::SHT_SYMTAB
            && (section.sh_entsize(endian) != 24 || section.sh_size(endian) % 24 != 0)
        {
            return Err(error("invalid ELF64 symbol table entry size"));
        }
        if let Some((flags, members)) = section.group(endian, file.data()).map_err(error)? {
            if flags & !elf::GRP_COMDAT != 0
                || section.link(endian) != file.elf_symbol_table().section()
                || section.sh_info(endian) == 0
                || members.is_empty()
            {
                return Err(error("invalid section group header"));
            }
            file.symbol_by_index(SymbolIndex(section.sh_info(endian) as usize))
                .map_err(error)?;
            for member in members {
                let member = SectionIndex(member.get(endian) as usize);
                let header = table.section(member).map_err(error)?;
                if member.0 == 0
                    || member == index
                    || !grouped.insert(member.0)
                    || header.sh_flags(endian) & u64::from(elf::SHF_GROUP) == 0
                {
                    return Err(error("invalid or repeated section group member"));
                }
            }
        }
    }
    for (index, section) in table.enumerate().skip(1) {
        if section.sh_flags(endian) & u64::from(elf::SHF_GROUP) != 0 && !grouped.contains(&index.0)
        {
            return Err(error("SHF_GROUP section has no group"));
        }
    }
    for symbol in file.symbols() {
        symbol.name_bytes().map_err(error)?;
        let Some(index) = symbol.section_index() else {
            continue;
        };
        let section = file.section_by_index(index).map_err(error)?;
        if symbol
            .address()
            .checked_add(symbol.size())
            .is_none_or(|end| end > section.size())
        {
            return Err(error("symbol extent is outside its section"));
        }
        if symbol.kind() == SymbolKind::Tls
            && section.elf_section_header().sh_flags(endian) & u64::from(elf::SHF_TLS) == 0
        {
            return Err(error("TLS symbol is not in TLS storage"));
        }
    }
    Ok(())
}
