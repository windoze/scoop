use super::*;
use object::Architecture;
use object::{ObjectComdat, SectionFlags, pe};

pub(super) fn read(bytes: &[u8]) -> Result<NativeObjectInfo, LinkError> {
    read_with_import_sections(bytes).map(|(info, _)| info)
}

pub(crate) fn read_with_import_sections(
    bytes: &[u8],
) -> Result<(NativeObjectInfo, bool), LinkError> {
    let file = object::File::parse(bytes).map_err(|err| {
        error(format!(
            "ordinary AMD64 COFF object required; MSVC /GL or /LTCG input is not supported: {err}"
        ))
    })?;
    if file.format() != object::BinaryFormat::Coff
        || file.architecture() != Architecture::X86_64
        || file.kind() != ObjectKind::Relocatable
        || !file.is_little_endian()
    {
        return Err(error(
            "native input must be an ordinary AMD64 COFF object (including bigobj), not MSVC /GL or /LTCG",
        ));
    }
    let mut coalescible = BTreeSet::new();
    for comdat in file.comdats() {
        if comdat.kind() != object::ComdatKind::NoDuplicates {
            coalescible.extend(comdat.sections().map(|section| section.0));
        }
    }
    let mut definitions = BTreeMap::new();
    let mut requirements = BTreeSet::new();
    let imports = file
        .sections()
        .any(|section| section.name().is_ok_and(|name| name.starts_with(".idata$")));
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
        let section = symbol
            .section_index()
            .map(|index| file.section_by_index(index).map_err(error))
            .transpose()?;
        let tls = section.as_ref().is_some_and(|section| {
            section
                .name()
                .is_ok_and(|name| name == ".tls" || name.starts_with(".tls$"))
        });
        let kind = if tls {
            NativeSymbolKind::ThreadLocal
        } else if symbol.kind() == SymbolKind::Text
            || (imports
                && section
                    .as_ref()
                    .is_some_and(|section| section.kind() == object::SectionKind::Text))
        {
            // GNU import thunks can omit COFF's function type on code symbols.
            NativeSymbolKind::Function
        } else {
            NativeSymbolKind::Data
        };
        let read_only = section.as_ref().is_some_and(|section| matches!(section.flags(), SectionFlags::Coff { characteristics } if characteristics & pe::IMAGE_SCN_MEM_WRITE == 0));
        let binding = NativeSymbolBinding::from_weak(
            symbol.is_weak()
                || symbol
                    .section_index()
                    .is_some_and(|index| coalescible.contains(&index.0)),
        );
        if definitions
            .insert(
                name.clone(),
                NativeSymbolDefinition {
                    kind,
                    read_only,
                    binding,
                },
            )
            .is_some()
        {
            return Err(error(format!("duplicate native COFF definition {name}")));
        }
    }
    Ok((
        NativeObjectInfo {
            definitions,
            requirements,
        },
        imports,
    ))
}
