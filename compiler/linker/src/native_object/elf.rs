use super::*;
use object::elf;
use object::read::elf::SectionHeader;
use scoop_slib::ValidatedElfObject;

pub(super) fn read(bytes: &[u8], target: TargetProfileId) -> Result<NativeObjectInfo, LinkError> {
    let index = index(bytes, target)?;
    index.check_selected(bytes)?;
    Ok(index.info)
}

pub(super) fn index(bytes: &[u8], target: TargetProfileId) -> Result<NativeObjectIndex, LinkError> {
    let object = ValidatedElfObject::read(bytes, target).map_err(error)?;
    let file = object.file();
    let mut definitions = BTreeMap::new();
    let mut requirements = BTreeSet::new();
    let mut rejection = None;
    for section in file.sections() {
        let name = section.name().map_err(error)?;
        let kind = section.elf_section_header().sh_type(file.endian());
        if matches!(
            kind,
            elf::SHT_INIT_ARRAY | elf::SHT_FINI_ARRAY | elf::SHT_PREINIT_ARRAY
        ) || matches!(name, ".init" | ".fini" | ".ctors" | ".dtors" | ".llvm.lto")
            || name.starts_with(".gnu.lto_")
            || (name == ".note.GNU-stack"
                && section.elf_section_header().sh_flags(file.endian())
                    & u64::from(elf::SHF_EXECINSTR)
                    != 0)
        {
            rejection.get_or_insert_with(|| format!(
                "native object contains unsupported initialization, LTO or executable-stack section {name}"
            ));
        }
    }
    for symbol in file.symbols().filter(|symbol| symbol.is_global()) {
        let name = symbol.name().map_err(error)?.to_owned();
        if (name.starts_with("__cxa_")
            && !matches!(name.as_ref(), "__cxa_finalize" | "__cxa_atexit"))
            || name.starts_with("__gxx_personality")
            || name.starts_with("__gcc_personality")
            || name.starts_with("_ZSt9terminate")
        {
            rejection.get_or_insert_with(|| {
                format!("native object has forbidden C++ EH dependency {name}")
            });
        }
        if symbol.is_common() {
            rejection.get_or_insert_with(|| {
                format!("native common/tentative definition {name} requires actual storage")
            });
            definitions.insert(
                name,
                NativeSymbolDefinition {
                    kind: NativeSymbolKind::Data,
                    read_only: false,
                    weak: symbol.is_weak(),
                },
            );
            continue;
        }
        if symbol.is_undefined() {
            requirements.insert(name);
            continue;
        }
        let kind = match symbol.kind() {
            SymbolKind::Text => NativeSymbolKind::Function,
            SymbolKind::Data => NativeSymbolKind::Data,
            SymbolKind::Tls => NativeSymbolKind::ThreadLocal,
            other => {
                return Err(error(format!(
                    "native definition {name} has invalid kind {other:?}"
                )));
            }
        };
        let section = file
            .section_by_index(
                symbol
                    .section_index()
                    .ok_or_else(|| error(format!("native definition {name} has no section")))?,
            )
            .map_err(error)?;
        let flags = section.elf_section_header().sh_flags(file.endian());
        if kind == NativeSymbolKind::Function && flags & u64::from(elf::SHF_EXECINSTR) == 0 {
            return Err(error(format!(
                "native function {name} has no executable storage"
            )));
        }
        let read_only = flags & u64::from(elf::SHF_WRITE) == 0
            || section.name().map_err(error)?.starts_with(".data.rel.ro");
        if definitions
            .insert(
                name.clone(),
                NativeSymbolDefinition {
                    kind,
                    read_only,
                    weak: symbol.is_weak(),
                },
            )
            .is_some()
        {
            return Err(error(format!("duplicate native definition {name}")));
        }
    }
    Ok(NativeObjectIndex {
        info: NativeObjectInfo {
            definitions,
            requirements,
        },
        selection: NativeSelectionChecks::Elf { rejection },
    })
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
