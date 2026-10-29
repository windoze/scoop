//! Symbol information used to select ordinary native objects.
use std::collections::{BTreeMap, BTreeSet};

use object::{
    Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, SymbolKind, macho,
    read::macho::MachOFile64,
};
use scoop_lir::{CBridgeToolchainProfileV1, DarwinCBridgeDeploymentContractV1};

use crate::{LinkError, error};

pub(crate) mod coff;
mod elf;
mod wire;

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeSymbolKind {
    Function,
    Data,
    ThreadLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeSymbolBinding {
    Weak,
    Common,
    Strong,
}

impl NativeSymbolBinding {
    pub(crate) fn precedence(self, format: scoop_lir::NativeObjectFormat) -> u8 {
        match (self, format) {
            (Self::Strong, _) => 2,
            (Self::Common, scoop_lir::NativeObjectFormat::Elf64)
            | (Self::Common, scoop_lir::NativeObjectFormat::Coff64)
            | (Self::Weak, scoop_lir::NativeObjectFormat::MachO64) => 1,
            _ => 0,
        }
    }

    pub(crate) fn from_weak(weak: bool) -> Self {
        if weak { Self::Weak } else { Self::Strong }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSymbolDefinition {
    pub kind: NativeSymbolKind,
    pub read_only: bool,
    pub binding: NativeSymbolBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeObjectInfo {
    pub definitions: BTreeMap<String, NativeSymbolDefinition>,
    pub requirements: BTreeSet<String>,
}

impl NativeObjectInfo {
    pub fn read_with_toolchain(
        bytes: &[u8],
        toolchain: &CBridgeToolchainProfileV1,
    ) -> Result<Self, LinkError> {
        match toolchain.contract().target().id() {
            scoop_target_data::DARWIN_AARCH64_ID => {
                Self::read(bytes, toolchain.contract().deployment().map_err(error)?)
            }
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
                elf::read(bytes)
            }
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID => coff::read(bytes),
            _ => Err(error("unsupported native object target")),
        }
    }

    pub fn read(
        bytes: &[u8],
        deployment: &DarwinCBridgeDeploymentContractV1,
    ) -> Result<Self, LinkError> {
        let file: MachOFile64<'_> = MachOFile64::parse(bytes).map_err(error)?;
        if file.architecture() != Architecture::Aarch64
            || file.kind() != ObjectKind::Relocatable
            || !file.is_little_endian()
            || file.macho_header().cpusubtype.get(file.endian()) != macho::CPU_SUBTYPE_ARM64_ALL
        {
            return Err(error(
                "native input must be an ordinary little-endian arm64 Mach-O object",
            ));
        }
        check_deployment(&file, deployment)?;
        let mut definitions = BTreeMap::new();
        let mut requirements = BTreeSet::new();
        for symbol in file.symbols().filter(|symbol| symbol.is_global()) {
            let name = symbol.name().map_err(error)?.to_owned();
            // Mach-O tentative storage uses N_UNDF with a nonzero n_value.
            if symbol.is_undefined() && symbol.address() != 0 {
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
                requirements.insert(name);
                continue;
            }
            let section = symbol
                .section_index()
                .map(|index| file.section_by_index(index).map_err(error))
                .transpose()?;
            let instructions = section.as_ref().is_some_and(|section| {
                section.macho_section().flags.get(file.endian())
                    & (macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS)
                    != 0
            });
            let kind = match symbol.kind() {
                SymbolKind::Text => NativeSymbolKind::Function,
                SymbolKind::Tls => NativeSymbolKind::ThreadLocal,
                SymbolKind::Unknown if instructions => NativeSymbolKind::Function,
                _ => NativeSymbolKind::Data,
            };
            let read_only = match &section {
                Some(section) => {
                    matches!(
                        section.kind(),
                        object::SectionKind::ReadOnlyData
                            | object::SectionKind::ReadOnlyString
                            | object::SectionKind::Text
                    ) || section.segment_name().map_err(error)? == Some("__DATA_CONST")
                }
                None => true,
            };
            if definitions
                .insert(
                    name.clone(),
                    NativeSymbolDefinition {
                        kind,
                        read_only,
                        binding: NativeSymbolBinding::from_weak(symbol.is_weak()),
                    },
                )
                .is_some()
            {
                return Err(error(format!("duplicate native definition {name}")));
            }
        }
        Ok(Self {
            definitions,
            requirements,
        })
    }
}

fn check_deployment(
    file: &MachOFile64<'_>,
    deployment: &DarwinCBridgeDeploymentContractV1,
) -> Result<(), LinkError> {
    let mut commands = file.macho_load_commands().map_err(error)?;
    let endian = file.endian();
    while let Some(command) = commands.next().map_err(error)? {
        if let Some(build) = command.build_version().map_err(error)?
            && (build.platform.get(endian) != macho::PLATFORM_MACOS
                || build.minos.get(endian) > deployment.minimum_os().packed())
        {
            return Err(error(
                "native object has incompatible platform or minimum macOS",
            ));
        }
    }
    Ok(())
}
