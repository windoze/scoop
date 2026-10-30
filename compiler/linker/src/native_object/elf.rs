use super::*;
use object::elf;
use object::read::elf::{ElfFile64, SectionHeader};

pub(super) fn read(
    bytes: &[u8],
    target: scoop_lir::TargetProfileId,
) -> Result<NativeObjectInfo, LinkError> {
    let machine = scoop_slib::ElfArchitecture::for_target(target).map_err(error)?;
    let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
    if file.architecture() != machine.object_architecture()
        || !file.is_little_endian()
        || file.kind() != ObjectKind::Relocatable
    {
        return Err(error(
            "native input must be a little-endian ELF64 object for the selected target",
        ));
    }
    let mut definitions = BTreeMap::new();
    let mut requirements = BTreeSet::new();
    for symbol in file.symbols().filter(|symbol| symbol.is_global()) {
        let name = symbol.name().map_err(error)?.to_owned();
        if symbol.is_common() {
            definitions.insert(
                name,
                NativeSymbolDefinition {
                    kind: NativeSymbolKind::Data,
                    read_only: false,
                    binding: NativeSymbolBinding::Common,
                },
            );
            continue;
        }
        if symbol.is_undefined() {
            if !symbol.is_weak() {
                requirements.insert(name);
            }
            continue;
        }
        let kind = match symbol.kind() {
            SymbolKind::Text => NativeSymbolKind::Function,
            SymbolKind::Tls => NativeSymbolKind::ThreadLocal,
            _ => NativeSymbolKind::Data,
        };
        let read_only = match symbol.section_index() {
            Some(index) => {
                let section = file.section_by_index(index).map_err(error)?;
                section.elf_section_header().sh_flags(file.endian()) & u64::from(elf::SHF_WRITE)
                    == 0
                    || section.name().map_err(error)?.starts_with(".data.rel.ro")
            }
            None => true,
        };
        if definitions
            .insert(
                name.clone(),
                NativeSymbolDefinition {
                    kind,
                    read_only,
                    binding: NativeSymbolBinding::from_weak(
                        symbol.is_weak() || symbol.elf_symbol().st_bind() == elf::STB_GNU_UNIQUE,
                    ),
                },
            )
            .is_some()
        {
            return Err(error(format!("duplicate native definition {name}")));
        }
    }
    Ok(NativeObjectInfo {
        definitions,
        requirements,
    })
}

#[cfg(all(
    test,
    any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )
))]
mod tests;
