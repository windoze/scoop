use super::*;
use crate::{NativeSymbolKind, macho_exports::ExportTarget};
mod commands;
use object::{Object, ObjectKind, ObjectSection, ObjectSymbol, macho, read::macho::MachOFile64};

pub(crate) fn read(
    bytes: &[u8],
    input: NativeInputId,
    locator: &Path,
    profile: &ValidatedFinalLinkProfile,
    text: bool,
    system: bool,
) -> Result<Vec<Arc<DynamicProvider>>, LinkError> {
    if text {
        return scoop_toolchain::read_text_stubs(
            bytes,
            profile
                .startup_toolchain()
                .profile()
                .contract()
                .deployment()
                .map_err(error)?
                .minimum_os(),
            system,
        )
        .map_err(error)?
        .into_iter()
        .map(|record| {
            let id = DynamicProvider::id(input, &record.install_name, profile)?;
            Ok(Arc::new(DynamicProvider {
                id,
                input,
                install_name: record.install_name,
                current_version: record.current_version,
                compatibility_version: record.compatibility_version,
                exports: record
                    .exports
                    .into_iter()
                    .map(|(name, interface)| {
                        (
                            name,
                            DynamicExport::Symbol {
                                interface,
                                storage: ExportStorage::InterfaceOnly,
                            },
                        )
                    })
                    .collect(),
                dependencies: record
                    .reexports
                    .into_iter()
                    .map(|name| LoadDependency {
                        name,
                        reexport: true,
                        compatibility_version: 0,
                    })
                    .collect(),
                rpaths: Vec::new(),
                locator: locator.to_owned(),
            }))
        })
        .collect();
    }
    let file = MachOFile64::parse(bytes).map_err(error)?;
    let endian = file.endian();
    if file.kind() != ObjectKind::Dynamic
        || !file.is_little_endian()
        || file.macho_header().cputype.get(endian) != macho::CPU_TYPE_ARM64
        || file.macho_header().cpusubtype.get(endian) != macho::CPU_SUBTYPE_ARM64_ALL
    {
        return Err(error(
            "native dynamic input must be a little-endian arm64 MH_DYLIB",
        ));
    }
    if file.macho_header().flags.get(endian) & macho::MH_TWOLEVEL == 0 {
        return Err(error(
            "native dylib uses flat namespace instead of two-level binding",
        ));
    }
    let commands = commands::read(&file, bytes, profile)?;
    let mut exports = BTreeMap::new();
    for (name, export) in crate::macho_exports::read(commands.trie)? {
        let value = match export.target {
            ExportTarget::Reexport { ordinal, imported } => {
                if ordinal == 0 || ordinal > commands.dependencies.len() {
                    return Err(error(format!(
                        "native re-export {name} has invalid dependency ordinal {ordinal}"
                    )));
                }
                DynamicExport::Reexport {
                    dependency: ordinal - 1,
                    imported,
                }
            }
            ExportTarget::Image {
                offset,
                thread_local,
            } => {
                let address = commands
                    .base
                    .checked_add(offset)
                    .ok_or_else(|| error("native export address overflow"))?;
                let section = file.sections().find(|section| {
                    address >= section.address() && address - section.address() < section.size()
                });
                let storage = if let Some(section) = section {
                    let kind = if thread_local {
                        NativeSymbolKind::ThreadLocal
                    } else if section.kind() == object::SectionKind::Text {
                        NativeSymbolKind::Function
                    } else {
                        NativeSymbolKind::Data
                    };
                    ExportStorage::Definition(NativeSymbolDefinition {
                        kind,
                        read_only: matches!(
                            section.kind(),
                            object::SectionKind::Text
                                | object::SectionKind::ReadOnlyData
                                | object::SectionKind::ReadOnlyString
                        ) || section.segment_name().map_err(error)?
                            == Some("__DATA_CONST"),
                        weak: export.weak,
                    })
                } else if thread_local {
                    ExportStorage::InterfaceOnly
                } else {
                    return Err(error(format!("native export {name} is outside its image")));
                };
                DynamicExport::Symbol {
                    interface: NativeExport {
                        kind: if thread_local {
                            SystemExportKind::ThreadLocal
                        } else {
                            SystemExportKind::Symbol
                        },
                        weak: export.weak,
                    },
                    storage,
                }
            }
            ExportTarget::Absolute(_) => {
                return Err(error(format!(
                    "native absolute export {name} has no function or data storage"
                )));
            }
        };
        exports.insert(name, value);
    }
    for symbol in file.symbols().filter(|symbol| symbol.is_undefined()) {
        let name = symbol.name().map_err(error)?;
        if name.starts_with("___cxa_")
            || name.starts_with("___gxx_personality")
            || name.starts_with("___gcc_personality")
        {
            return Err(error(format!(
                "native dylib has forbidden C++ ABI dependency {name}"
            )));
        }
        let ordinal = ((symbol.macho_symbol().n_desc.get(endian) >> 8) & 255) as usize;
        if ordinal == 0 || ordinal > commands.dependencies.len() {
            return Err(error(format!(
                "native dylib import {name} has no explicit dependency ordinal"
            )));
        }
    }
    let id = DynamicProvider::id(input, &commands.name, profile)?;
    Ok(vec![Arc::new(DynamicProvider {
        id,
        input,
        install_name: commands.name,
        current_version: commands.current,
        compatibility_version: commands.compatibility,
        exports,
        dependencies: commands.dependencies,
        rpaths: commands.rpaths,
        locator: locator.to_owned(),
    })])
}
