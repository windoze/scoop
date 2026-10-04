//! Ordinary target object checks shared by runtime and startup inputs.
use std::collections::{BTreeMap, BTreeSet};

use object::{
    Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, SymbolKind, macho,
    read::macho::MachOFile64,
};
use scoop_lir::{CBridgeToolchainProfileV1, DarwinCBridgeDeploymentContractV1, TargetProfileId};

use crate::{LinkError, error};

mod elf;
mod references;
mod sections;
mod selected;
pub(crate) use references::{NativeReferenceSection, NativeReferences};
mod wire;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeSymbolKind {
    Function,
    Data,
    ThreadLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSymbolDefinition {
    pub kind: NativeSymbolKind,
    pub read_only: bool,
    pub weak: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeObjectInfo {
    pub definitions: BTreeMap<String, NativeSymbolDefinition>,
    pub requirements: BTreeSet<String>,
}

pub(crate) struct NativeObjectIndex {
    pub info: NativeObjectInfo,
    common: BTreeSet<String>,
    implicit_inputs: Vec<u32>,
}

impl NativeObjectInfo {
    pub fn read_with_toolchain(
        bytes: &[u8],
        toolchain: &CBridgeToolchainProfileV1,
    ) -> Result<Self, LinkError> {
        match toolchain.contract().target().id() {
            TargetProfileId::DarwinAarch64 => {
                Self::read(bytes, toolchain.contract().deployment().map_err(error)?)
            }
            target @ (TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl) => {
                elf::read(bytes, target)
            }
        }
    }

    pub fn read(
        bytes: &[u8],
        deployment: &DarwinCBridgeDeploymentContractV1,
    ) -> Result<Self, LinkError> {
        let index = NativeObjectIndex::read(bytes, deployment)?;
        index.check_selected(bytes)?;
        Ok(index.info)
    }
}

impl NativeObjectIndex {
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
        let implicit_inputs = check_commands(&file, deployment)?;
        let mut common = BTreeSet::new();
        let mut definitions = BTreeMap::new();
        let mut requirements = BTreeSet::new();
        for symbol in file.symbols() {
            if !symbol.is_global() {
                continue;
            }
            let name = symbol.name().map_err(error)?.to_owned();
            // Mach-O tentative storage uses N_UNDF with a nonzero n_value.
            if symbol.is_undefined() && symbol.address() != 0 {
                common.insert(name.clone());
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
            let end = section
                .address()
                .checked_add(section.size())
                .ok_or_else(|| error("native section address overflow"))?;
            if !(section.address()..=end).contains(&symbol.address()) {
                return Err(error(format!(
                    "native definition {name} is outside its section"
                )));
            }
            let read_only = matches!(
                section.kind(),
                object::SectionKind::ReadOnlyData
                    | object::SectionKind::ReadOnlyString
                    | object::SectionKind::Text
            ) || section.segment_name().map_err(error)? == Some("__DATA_CONST");
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
        Ok(Self {
            info: NativeObjectInfo {
                definitions,
                requirements,
            },
            common,
            implicit_inputs,
        })
    }
}

fn check_commands(
    file: &MachOFile64<'_>,
    deployment: &DarwinCBridgeDeploymentContractV1,
) -> Result<Vec<u32>, LinkError> {
    let mut implicit_inputs = Vec::new();
    let mut commands = file.macho_load_commands().map_err(error)?;
    let mut build_count = 0;
    let endian = file.endian();
    while let Some(command) = commands.next().map_err(error)? {
        match command.cmd() {
            macho::LC_BUILD_VERSION => {
                let build = command
                    .build_version()
                    .map_err(error)?
                    .ok_or_else(|| error("invalid native LC_BUILD_VERSION"))?;
                if build.platform.get(endian) != macho::PLATFORM_MACOS
                    || build.minos.get(endian) > deployment.minimum_os().packed()
                {
                    return Err(error(
                        "native object has incompatible platform or minimum macOS",
                    ));
                }
                build_count += 1;
            }
            macho::LC_LINKER_OPTION
            | macho::LC_LOAD_DYLIB
            | macho::LC_LOAD_WEAK_DYLIB
            | macho::LC_REEXPORT_DYLIB => {
                implicit_inputs.push(command.cmd());
            }
            macho::LC_SEGMENT_64
            | macho::LC_SYMTAB
            | macho::LC_DYSYMTAB
            | macho::LC_DATA_IN_CODE
            | macho::LC_LINKER_OPTIMIZATION_HINT
            | macho::LC_FUNCTION_STARTS
            | macho::LC_SOURCE_VERSION
            | macho::LC_UUID => {}
            other => implicit_inputs.push(other),
        }
    }
    if build_count != 1 {
        return Err(error("native object requires one LC_BUILD_VERSION"));
    }
    Ok(implicit_inputs)
}
