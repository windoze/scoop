use super::*;
use crate::native_input::NativeFileKind;
use object::read::archive::ArchiveFile;
use object::{Architecture, ObjectKind, ObjectSection, ObjectSymbol, SectionFlags};

impl ElfNamespace {
    pub(super) fn read_system(
        &mut self,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<(), LinkError> {
        for path in &self.paths {
            // Driver executables and CRT objects are not source extern providers.
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if !filename.ends_with(".a") && !filename.contains(".so") {
                continue;
            }
            let bytes = std::fs::read(path).map_err(error)?;
            let mut symbols = BTreeMap::new();
            let (kind, name) = if bytes.starts_with(b"!<arch>\n") {
                let archive = ArchiveFile::parse(bytes.as_slice()).map_err(error)?;
                for member in archive.members() {
                    let member = member.map_err(error)?;
                    symbols.extend(exports(member.data(bytes.as_slice()).map_err(error)?)?);
                }
                (NativeFileKind::Archive, filename.to_owned())
            } else if bytes.starts_with(b"\x7fELF") {
                let interface = ElfDynamic::read(&bytes, profile.id())?;
                let name = interface.soname.as_deref().unwrap_or(filename).to_owned();
                for (symbol, exports) in &interface.symbols.exports {
                    if let Some(export) = exports.iter().find(|export| export.default) {
                        symbols.insert(symbol.clone(), export.definition);
                    }
                }
                self.needed.insert(name.clone());
                self.system_interfaces
                    .insert(name.clone(), interface.symbols);
                (NativeFileKind::SharedObject, name)
            } else {
                // Development .so scripts name concrete ELF files in the same
                // driver trace; their bytes are already part of the profile.
                continue;
            };
            let id = NativeInputId::from_bytes(&bytes, kind, profile)?;
            if matches!(name.as_str(), "libc.a" | "libc.so" | "libc.so.6") {
                self.libc_inputs.insert(id);
            }
            self.system.extend(symbols.clone());
            self.system_inputs.insert(id, symbols);
        }
        Ok(())
    }
}

fn exports(bytes: &[u8]) -> Result<BTreeMap<String, NativeSymbolDefinition>, LinkError> {
    let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
    if file.architecture() != Architecture::X86_64
        || !file.is_little_endian()
        || file.kind() != ObjectKind::Relocatable
    {
        return Err(error(
            "selected system input is not the expected ELF64 amd64 object",
        ));
    }
    let symbols = file.symbols();
    let mut result = BTreeMap::new();
    for symbol in symbols {
        if !symbol.is_global() || symbol.is_undefined() {
            continue;
        }
        let raw = symbol.elf_symbol();
        let kind = match raw.st_type() {
            elf::STT_FUNC | elf::STT_GNU_IFUNC => NativeSymbolKind::Function,
            elf::STT_OBJECT | elf::STT_COMMON => NativeSymbolKind::Data,
            elf::STT_TLS => NativeSymbolKind::ThreadLocal,
            // Absolute version-node symbols are not addressable C definitions.
            _ => continue,
        };
        let read_only = match symbol.section_index() {
            Some(index) => match file.section_by_index(index).map_err(error)?.flags() {
                SectionFlags::Elf { sh_flags } => sh_flags & u64::from(elf::SHF_WRITE) == 0,
                _ => return Err(error("non-ELF section in ELF input")),
            },
            None => false,
        };
        result.insert(
            symbol.name().map_err(error)?.to_owned(),
            NativeSymbolDefinition {
                kind,
                read_only,
                weak: symbol.is_weak(),
            },
        );
    }
    Ok(result)
}
