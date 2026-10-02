//! Ordinary Mach-O object checks shared by runtime and startup inputs.
use std::collections::{BTreeMap, BTreeSet};

use object::{
    Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, RelocationTarget, SymbolKind,
    macho, read::macho::MachOFile64,
};
use scoop_lir::DarwinCBridgeDeploymentContractV1;

use crate::{LinkError, error};

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

impl NativeObjectInfo {
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
        check_commands(&file, deployment)?;
        for section in file.sections() {
            let name = section.name().map_err(error)?;
            if matches!(
                name,
                "__mod_init_func" | "__mod_term_func" | "__thread_init" | "__init_offsets"
            ) || section.segment_name().map_err(error)? == Some("__LLVM")
                || name.starts_with("__objc_")
                || name.starts_with("__swift")
            {
                return Err(error(format!(
                    "native object contains forbidden initialization or LTO section {name}"
                )));
            }
            section.data().map_err(error)?;
            for (offset, relocation) in section.relocations() {
                if offset
                    .checked_add(u64::from(relocation.size().div_ceil(8)))
                    .is_none_or(|end| end > section.size())
                {
                    return Err(error(format!("native relocation outside {name}")));
                }
                match relocation.target() {
                    RelocationTarget::Symbol(index) => {
                        file.symbol_by_index(index).map_err(error)?;
                    }
                    RelocationTarget::Section(index) => {
                        file.section_by_index(index).map_err(error)?;
                    }
                    RelocationTarget::Absolute => {}
                    _ => return Err(error("unknown native relocation target")),
                }
            }
        }
        let mut definitions = BTreeMap::new();
        let mut requirements = BTreeSet::new();
        for symbol in file.symbols() {
            if !symbol.is_global() {
                continue;
            }
            let name = symbol.name().map_err(error)?.to_owned();
            if name.starts_with("___cxa_")
                || name.starts_with("___gxx_personality")
                || name.starts_with("___gcc_personality")
                || name.starts_with("__ZSt9terminate")
            {
                return Err(error(format!(
                    "native object has forbidden C++ ABI dependency {name}"
                )));
            }
            // In Mach-O, an external N_UNDF with a nonzero value is tentative
            // storage; object::ObjectSymbol::is_common() does not expose it.
            if symbol.is_undefined() && symbol.address() != 0 {
                return Err(error(format!(
                    "native common/tentative definition {name} requires actual storage"
                )));
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
            definitions,
            requirements,
        })
    }
}

fn check_commands(
    file: &MachOFile64<'_>,
    deployment: &DarwinCBridgeDeploymentContractV1,
) -> Result<(), LinkError> {
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
                return Err(error("native object requests an implicit linker input"));
            }
            _ => {}
        }
    }
    if build_count != 1 {
        return Err(error("native object requires one LC_BUILD_VERSION"));
    }
    Ok(())
}
