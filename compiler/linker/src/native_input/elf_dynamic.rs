//! ELF shared-library interfaces, including GNU symbol versions.
use std::collections::BTreeMap;

use object::read::elf::{Dyn, ElfFile64, ProgramHeader};
use object::{Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionFlags, elf};

use crate::{LinkError, NativeSymbolDefinition, NativeSymbolKind, error};

mod symbols;
pub(crate) use symbols::read_symbols;

#[derive(Clone, Debug)]
pub(crate) struct ElfExport {
    pub definition: NativeSymbolDefinition,
    pub version: Option<String>,
    pub default: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ElfImport {
    pub name: String,
    pub version: Option<String>,
    pub library: Option<String>,
    pub weak: bool,
}

#[derive(Default)]
pub(crate) struct ElfSymbols {
    pub exports: BTreeMap<String, Vec<ElfExport>>,
    pub imports: Vec<ElfImport>,
}

impl ElfSymbols {
    pub fn find(&self, symbol: &str, version: Option<&str>) -> Option<&ElfExport> {
        self.exports
            .get(symbol)?
            .iter()
            .find(|export| match version {
                Some(version) => export.version.as_deref() == Some(version),
                None => export.default,
            })
    }
}

pub(crate) struct ElfDynamic {
    pub soname: Option<String>,
    pub needed: Vec<String>,
    pub runpaths: Vec<String>,
    pub symbols: ElfSymbols,
}

impl ElfDynamic {
    pub fn read(bytes: &[u8], target: scoop_lir::TargetProfileId) -> Result<Self, LinkError> {
        let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
        let architecture = match target {
            scoop_lir::TargetProfileId::LinuxX86_64Gnu
            | scoop_lir::TargetProfileId::LinuxX86_64Musl => Architecture::X86_64,
            scoop_lir::TargetProfileId::DarwinAarch64 => {
                return Err(error("ELF DSO requires an ELF target"));
            }
        };
        if file.architecture() != architecture
            || !file.is_little_endian()
            || file.kind() != ObjectKind::Dynamic
        {
            return Err(error("native shared library must be an ELF64 amd64 DSO"));
        }
        let mut has_dynamic = false;
        for segment in file.elf_program_headers() {
            if segment.p_type(file.endian()) == elf::PT_GNU_STACK
                && segment.p_flags(file.endian()) & elf::PF_X != 0
            {
                return Err(error("native ELF DSO requests an executable stack"));
            }
            if let Some(entries) = segment.dynamic(file.endian(), bytes).map_err(error)? {
                has_dynamic = true;
                for entry in entries {
                    let tag = entry.d_tag(file.endian());
                    if tag == u64::from(elf::DT_NULL) {
                        break;
                    }
                    if tag == u64::from(elf::DT_FLAGS_1)
                        && entry.d_val(file.endian()) & u64::from(elf::DF_1_PIE) != 0
                    {
                        return Err(error("native shared library is a PIE executable"));
                    }
                }
            }
        }
        if !has_dynamic {
            return Err(error("native ELF DSO has no dynamic table"));
        }
        let mut result = Self {
            soname: None,
            needed: Vec::new(),
            runpaths: Vec::new(),
            symbols: read_symbols(&file)?,
        };
        let mut rpath = Vec::new();
        let mut runpath = None;
        for (tag, value) in dynamic_strings(&file)? {
            match tag {
                elf::DT_SONAME => {
                    if value.is_empty() || result.soname.replace(value).is_some() {
                        return Err(error("empty or duplicate ELF SONAME"));
                    }
                }
                elf::DT_NEEDED => {
                    if value.is_empty() || result.needed.contains(&value) {
                        return Err(error("empty or duplicate ELF dependency"));
                    }
                    result.needed.push(value);
                }
                elf::DT_RPATH => rpath = value.split(':').map(str::to_owned).collect(),
                elf::DT_RUNPATH => runpath = Some(value.split(':').map(str::to_owned).collect()),
                _ => {}
            }
        }
        result.runpaths = runpath.unwrap_or(rpath);
        Ok(result)
    }
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
                if matches!(tag, value if value == u64::from(elf::DT_SONAME) || value == u64::from(elf::DT_NEEDED) || value == u64::from(elf::DT_RPATH) || value == u64::from(elf::DT_RUNPATH))
                {
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

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
