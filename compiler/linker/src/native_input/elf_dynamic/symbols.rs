use super::*;

pub(crate) fn read_symbols(file: &ElfFile64<'_>) -> Result<ElfSymbols, LinkError> {
    let endian = file.endian();
    let sections = file.elf_section_table();
    let versions = sections.versions(endian, file.data()).map_err(error)?;
    if let Some((indices, table)) = sections.gnu_versym(endian, file.data()).map_err(error)?
        && (indices.len() != file.elf_dynamic_symbol_table().len()
            || table != file.elf_dynamic_symbol_table().section())
    {
        return Err(error(
            "ELF symbol versions do not match the dynamic symbol table",
        ));
    }
    let mut result = ElfSymbols::default();
    for symbol in file.dynamic_symbols() {
        if !symbol.is_global() {
            continue;
        }
        let raw = symbol.elf_symbol();
        let name = symbol.name().map_err(error)?.to_owned();
        if name.is_empty() {
            continue;
        }
        let (version, library, default) = if let Some(versions) = &versions {
            let index = versions.version_index(endian, symbol.index());
            if index.is_local() && !symbol.is_undefined() {
                continue;
            }
            let version = versions.version(index).map_err(error)?;
            (
                version.map(|v| text(v.name())).transpose()?,
                version.and_then(|v| v.file()).map(text).transpose()?,
                !index.is_hidden(),
            )
        } else {
            (None, None, true)
        };
        if symbol.is_undefined() {
            result.imports.push(ElfImport {
                name,
                version,
                library,
                weak: symbol.is_weak(),
            });
            continue;
        }
        if !matches!(raw.st_visibility(), elf::STV_DEFAULT | elf::STV_PROTECTED) {
            continue;
        }
        let kind = match raw.st_type() {
            elf::STT_FUNC | elf::STT_GNU_IFUNC => NativeSymbolKind::Function,
            elf::STT_OBJECT => NativeSymbolKind::Data,
            elf::STT_TLS => NativeSymbolKind::ThreadLocal,
            _ => continue,
        };
        let Some(index) = symbol.section_index() else {
            continue;
        };
        let section = file.section_by_index(index).map_err(error)?;
        let SectionFlags::Elf { sh_flags } = section.flags() else {
            return Err(error("non-ELF section in ELF DSO"));
        };
        if sh_flags & u64::from(elf::SHF_ALLOC) == 0 {
            return Err(error(format!("ELF export {name} has no allocated storage")));
        }
        // TLS symbol values are offsets in PT_TLS, not ordinary image VAs.
        let relro = kind != NativeSymbolKind::ThreadLocal
            && file.elf_program_headers().iter().any(|segment| {
                segment.p_type(endian) == elf::PT_GNU_RELRO
                    && symbol
                        .address()
                        .checked_sub(segment.p_vaddr(endian))
                        .and_then(|offset| offset.checked_add(symbol.size()))
                        .is_some_and(|end| end <= segment.p_memsz(endian))
            });
        let export = ElfExport {
            definition: NativeSymbolDefinition {
                kind,
                read_only: sh_flags & u64::from(elf::SHF_WRITE) == 0 || relro,
                weak: symbol.is_weak(),
            },
            version,
            default,
        };
        let exports = result.exports.entry(name.clone()).or_default();
        if exports.iter().any(|previous| {
            previous.version == export.version || (previous.default && export.default)
        }) {
            return Err(error(format!("duplicate ELF export {name} version")));
        }
        exports.push(export);
    }
    Ok(result)
}

fn text(bytes: &[u8]) -> Result<String, LinkError> {
    std::str::from_utf8(bytes).map(str::to_owned).map_err(error)
}
