use super::*;
pub(crate) use crate::native_input::elf_dynamic::dynamic_strings;
use crate::native_input::elf_dynamic::{ElfDynamic, ElfExport, ElfImport, ElfSymbols};
use object::read::elf::ElfFile64;
use object::{Object, elf};
use std::collections::BTreeSet;
use std::sync::Arc;

mod final_image;
mod graph;
mod plan;
mod system;
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
    pub system_inputs: BTreeMap<NativeInputId, BTreeMap<String, NativeSymbolDefinition>>,
    pub libc_inputs: BTreeSet<NativeInputId>,
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
            system_inputs: BTreeMap::new(),
            libc_inputs: BTreeSet::new(),
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
        result.read_system(profile)?;
        result.read_graph(native, roots, profile)?;
        Ok(result)
    }

    pub fn candidates(
        &self,
        symbol: &str,
        explicit: Option<&[NativeInputId]>,
        system_alias: bool,
    ) -> Vec<NativeBinding> {
        let mut result = Vec::new();
        for id in self
            .roots
            .iter()
            .filter(|id| explicit.is_none_or(|explicit| explicit.contains(id)))
        {
            if let Some(export) = self.providers[id].interface.symbols.find(symbol, None) {
                result.push(NativeBinding::Elf(ElfBinding::Shared {
                    input: *id,
                    export: export.clone(),
                }));
            }
        }
        let definition = match explicit {
            None => self.system.get(symbol),
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.system_inputs.get(id))
                .find_map(|symbols| symbols.get(symbol)),
        }
        .or_else(|| {
            system_alias.then(|| {
                self.libc_inputs
                    .iter()
                    .find_map(|id| self.system_inputs[id].get(symbol))
            })?
        });
        if let Some(definition) = definition {
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
