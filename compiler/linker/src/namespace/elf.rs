use super::*;
use object::read::archive::ArchiveFile;
use object::read::elf::{Dyn, ElfFile64, ProgramHeader};
use object::{Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionFlags, elf};
use scoop_toolchain::LinuxFinalLinkProfile;
use std::collections::BTreeSet;

pub(crate) struct ElfNamespace {
    pub system: BTreeMap<String, NativeSymbolDefinition>,
    pub bindings: BTreeMap<String, NativeSymbolDefinition>,
    pub paths: Vec<PathBuf>,
    pub needed: BTreeSet<String>,
}

impl ElfNamespace {
    pub fn read(profile: &LinuxFinalLinkProfile) -> Result<Self, LinkError> {
        let mut result = Self {
            system: BTreeMap::new(),
            bindings: BTreeMap::new(),
            paths: profile.input_paths().map(PathBuf::from).collect(),
            needed: BTreeSet::new(),
        };
        for path in &result.paths {
            // Driver executables and CRT objects are not source extern providers.
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if !name.ends_with(".a") && !name.contains(".so") {
                continue;
            }
            let bytes = std::fs::read(path).map_err(error)?;
            if bytes.starts_with(b"!<arch>\n") {
                let archive = ArchiveFile::parse(bytes.as_slice()).map_err(error)?;
                for member in archive.members() {
                    let member = member.map_err(error)?;
                    let data = member.data(bytes.as_slice()).map_err(error)?;
                    result.system.extend(exports(data, false)?);
                }
            } else if bytes.starts_with(b"\x7fELF") {
                result.system.extend(exports(&bytes, true)?);
                let file: ElfFile64<'_> = ElfFile64::parse(bytes.as_slice()).map_err(error)?;
                let mut soname = false;
                for (tag, value) in dynamic_strings(&file)? {
                    if tag == elf::DT_SONAME {
                        soname = true;
                        result.needed.insert(value);
                    }
                }
                if !soname {
                    result.needed.insert(name.to_owned());
                }
            }
            // GNU libc development .so scripts name the concrete ELF files in
            // the same driver trace. Their text is already part of the profile.
        }
        Ok(result)
    }

    pub fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        // Actual CRT/library bytes are covered by the final toolchain profile.
        e.array(2)?;
        e.unsigned(2)?;
        e.array(self.bindings.len() as u64)?;
        for symbol in self.bindings.keys() {
            e.text(symbol)?;
        }
        Ok(())
    }
}

fn exports(
    bytes: &[u8],
    dynamic: bool,
) -> Result<BTreeMap<String, NativeSymbolDefinition>, LinkError> {
    let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
    if file.architecture() != Architecture::X86_64
        || !file.is_little_endian()
        || file.kind()
            != if dynamic {
                ObjectKind::Dynamic
            } else {
                ObjectKind::Relocatable
            }
    {
        return Err(error(
            "selected system input is not the expected ELF64 amd64 object",
        ));
    }
    let endian = file.endian();
    let sections = file.elf_section_table();
    let versions = if dynamic {
        sections.versions(endian, bytes).map_err(error)?
    } else {
        None
    };
    if dynamic
        && let Some((indices, table)) = sections.gnu_versym(endian, bytes).map_err(error)?
        && (indices.len() != file.elf_dynamic_symbol_table().len()
            || table != file.elf_dynamic_symbol_table().section())
    {
        return Err(error(
            "ELF symbol versions do not match the dynamic symbol table",
        ));
    }
    let symbols = if dynamic {
        file.dynamic_symbols()
    } else {
        file.symbols()
    };
    let mut result = BTreeMap::new();
    for symbol in symbols {
        if !symbol.is_global() || symbol.is_undefined() {
            continue;
        }
        let raw = symbol.elf_symbol();
        if dynamic && !matches!(raw.st_visibility(), elf::STV_DEFAULT | elf::STV_PROTECTED) {
            continue;
        }
        if let Some(versions) = &versions {
            let index = versions.version_index(endian, symbol.index());
            if index.is_local() || index.is_hidden() {
                continue;
            }
            versions.version(index).map_err(error)?;
        }
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

pub(crate) fn dynamic_strings(file: &ElfFile64<'_>) -> Result<Vec<(u32, String)>, LinkError> {
    let mut values = Vec::new();
    for segment in file.elf_program_headers() {
        if let Some(entries) = segment.dynamic(file.endian(), file.data()).map_err(error)? {
            for entry in entries {
                let tag = entry.d_tag(file.endian());
                if tag == u64::from(elf::DT_NULL) {
                    break;
                }
                if matches!(
                    tag as u32,
                    elf::DT_SONAME | elf::DT_NEEDED | elf::DT_RPATH | elf::DT_RUNPATH
                ) {
                    let offset = u32::try_from(entry.d_val(file.endian())).map_err(error)?;
                    let value = file
                        .elf_dynamic_symbol_table()
                        .strings()
                        .get(offset)
                        .map_err(|_| error("ELF dynamic string exceeds its table"))?;
                    values.push((
                        tag as u32,
                        std::str::from_utf8(value).map_err(error)?.to_owned(),
                    ));
                }
            }
        }
    }
    Ok(values)
}
