use super::*;
pub(crate) use crate::native_input::elf_dynamic::dynamic_strings;
use crate::native_input::elf_dynamic::{ElfDynamic, ElfExport, ElfImport, ElfSymbols};
use object::read::archive::ArchiveFile;
use object::read::elf::ElfFile64;
use object::{Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, SectionFlags, elf};
use std::collections::BTreeSet;
use std::sync::Arc;

mod final_image;
mod graph;
mod plan;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;

#[derive(Clone)]
pub(crate) enum ElfBinding {
    System(NativeSymbolDefinition),
    Shared {
        input: NativeInputId,
        export: ElfExport,
    },
}
impl ElfBinding {
    pub fn definition(&self) -> NativeSymbolDefinition {
        match self {
            Self::System(definition) => *definition,
            Self::Shared { export, .. } => export.definition,
        }
    }
}

pub(crate) struct ElfProvider {
    pub name: String,
    pub locator: PathBuf,
    pub interface: Arc<ElfDynamic>,
}

pub(crate) struct ElfNamespace {
    pub system: BTreeMap<String, NativeSymbolDefinition>,
    pub bindings: BTreeMap<String, ElfBinding>,
    pub system_interfaces: BTreeMap<String, ElfSymbols>,
    pub providers: BTreeMap<NativeInputId, ElfProvider>,
    pub roots: Vec<NativeInputId>,
    pub selected: BTreeSet<NativeInputId>,
    pub expanded: BTreeSet<NativeInputId>,
    pub edges: BTreeMap<NativeInputId, Vec<NativeInputId>>,
    pub rpaths: BTreeSet<PathBuf>,
    pub paths: Vec<PathBuf>,
    pub needed: BTreeSet<String>,
}

impl ElfNamespace {
    pub fn read(
        native: &mut NativeInputs,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        let ValidatedFinalLinkProfile::Linux(linux) = profile else {
            return Err(error("ELF namespace requires a Linux target"));
        };
        let mut result = Self {
            system: BTreeMap::new(),
            system_interfaces: BTreeMap::new(),
            providers: BTreeMap::new(),
            roots: Vec::new(),
            selected: BTreeSet::new(),
            expanded: BTreeSet::new(),
            edges: BTreeMap::new(),
            rpaths: BTreeSet::new(),
            bindings: BTreeMap::new(),
            paths: linux.input_paths().map(PathBuf::from).collect(),
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
                    result.system.extend(exports(data)?);
                }
            } else if bytes.starts_with(b"\x7fELF") {
                let interface = ElfDynamic::read(&bytes, profile.id())?;
                let name = interface.soname.as_deref().unwrap_or(name).to_owned();
                for (symbol, exports) in &interface.symbols.exports {
                    if let Some(export) = exports.iter().find(|export| export.default) {
                        result.system.insert(symbol.clone(), export.definition);
                    }
                }
                result.needed.insert(name.clone());
                result.system_interfaces.insert(name, interface.symbols);
            }
            // GNU libc development .so scripts name the concrete ELF files in
            // the same driver trace. Their text is already part of the profile.
        }
        result.read_graph(native, roots, profile)?;
        Ok(result)
    }

    pub fn candidates(&self, symbol: &str, explicit: Option<NativeInputId>) -> Vec<NativeBinding> {
        let mut result = Vec::new();
        for id in self
            .roots
            .iter()
            .filter(|id| explicit.is_none_or(|explicit| explicit == **id))
        {
            if let Some(export) = self.providers[id].interface.symbols.find(symbol, None) {
                result.push(NativeBinding::Elf(ElfBinding::Shared {
                    input: *id,
                    export: export.clone(),
                }));
            }
        }
        if explicit.is_none()
            && let Some(definition) = self.system.get(symbol)
        {
            result.push(NativeBinding::Elf(ElfBinding::System(*definition)));
        }
        result
    }

    pub fn system_import(&self, import: &ElfImport) -> bool {
        if import.version.is_none() {
            return self.system.contains_key(&import.name);
        }
        if let Some(library) = &import.library {
            return self.system_interfaces.get(library).is_some_and(|symbols| {
                symbols
                    .find(&import.name, import.version.as_deref())
                    .is_some()
            });
        }
        self.system_interfaces.values().any(|symbols| {
            symbols
                .find(&import.name, import.version.as_deref())
                .is_some()
        })
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
