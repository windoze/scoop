//! Platform-native symbol namespaces; ELF does not use Mach-O load ordinals.
use std::collections::BTreeMap;
use std::path::PathBuf;

use scoop_toolchain::{SystemExportKind, ValidatedFinalLinkProfile};
use scoop_wire::Encoder;

use crate::dynamic::{DynamicBinding, DynamicInputs, ExportStorage};
use crate::native_input::{NativeInputId, NativeInputs};
use crate::{LinkError, NativeSymbolDefinition, NativeSymbolKind, error};

pub(crate) mod elf;
pub(crate) use elf::ElfNamespace;

pub(crate) struct DarwinNamespace {
    pub providers: DynamicInputs,
    pub bindings: BTreeMap<String, DynamicBinding>,
}

pub(crate) enum NativeNamespace {
    Darwin(DarwinNamespace),
    Elf(ElfNamespace),
}

pub(crate) enum NativeBinding {
    Darwin(DynamicBinding),
    Elf(elf::ElfBinding),
}

impl NativeBinding {
    pub fn definition(&self) -> Option<NativeSymbolDefinition> {
        match self {
            Self::Darwin(binding) => match binding.storage {
                ExportStorage::Definition(definition) => Some(definition),
                ExportStorage::InterfaceOnly => None,
            },
            Self::Elf(binding) => Some(binding.definition()),
        }
    }

    pub fn is_tls(&self) -> bool {
        match self {
            Self::Darwin(binding) => binding.interface.kind == SystemExportKind::ThreadLocal,
            Self::Elf(binding) => binding.definition().kind == NativeSymbolKind::ThreadLocal,
        }
    }
}

impl NativeNamespace {
    pub fn read(
        native: &mut NativeInputs,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        match profile {
            ValidatedFinalLinkProfile::Darwin(_) => Ok(Self::Darwin(DarwinNamespace {
                providers: DynamicInputs::read(native, roots, profile)?,
                bindings: BTreeMap::new(),
            })),
            ValidatedFinalLinkProfile::Linux(_) => {
                ElfNamespace::read(native, roots, profile).map(Self::Elf)
            }
        }
    }

    pub fn darwin(&self) -> Result<&DarwinNamespace, LinkError> {
        match self {
            Self::Darwin(namespace) => Ok(namespace),
            Self::Elf(_) => Err(error("Mach-O operation requires a Darwin namespace")),
        }
    }

    pub fn candidates(
        &self,
        native: &NativeInputs,
        symbol: &str,
        explicit: Option<NativeInputId>,
    ) -> Result<Vec<NativeBinding>, LinkError> {
        match self {
            Self::Darwin(namespace) => namespace
                .providers
                .candidates(native, symbol, explicit)
                .map(|values| values.into_iter().map(NativeBinding::Darwin).collect()),
            Self::Elf(namespace) => Ok(namespace.candidates(symbol, explicit)),
        }
    }

    pub fn bind(
        &mut self,
        symbol: String,
        binding: NativeBinding,
    ) -> Result<Vec<String>, LinkError> {
        let mut requirements = Vec::new();
        match (self, binding) {
            (Self::Darwin(namespace), NativeBinding::Darwin(binding)) => {
                namespace.bindings.insert(symbol, binding);
            }
            (Self::Elf(namespace), NativeBinding::Elf(binding)) => {
                requirements = namespace.import_requirements(&binding);
                namespace.bindings.insert(symbol, binding);
            }
            _ => {
                return Err(error(
                    "native binding and namespace use different object formats",
                ));
            }
        }
        Ok(requirements)
    }

    pub fn bound_symbols(&self) -> Vec<&String> {
        match self {
            Self::Darwin(namespace) => namespace.bindings.keys().collect(),
            Self::Elf(namespace) => namespace.bindings.keys().collect(),
        }
    }

    pub fn unbind(&mut self, symbol: &str) {
        match self {
            Self::Darwin(namespace) => {
                namespace.bindings.remove(symbol);
            }
            Self::Elf(namespace) => {
                namespace.bindings.remove(symbol);
            }
        }
    }

    pub fn project(
        &mut self,
        definitions: &BTreeMap<String, crate::program::DefinitionOwner>,
        roots: &[PathBuf],
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<(), LinkError> {
        match self {
            Self::Darwin(namespace) => {
                namespace
                    .providers
                    .project(&namespace.bindings, roots, profile)
            }
            Self::Elf(namespace) => namespace.project(definitions),
        }
    }

    pub fn input_paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Darwin(namespace) => namespace
                .providers
                .providers
                .values()
                .map(|p| p.locator.clone())
                .collect(),
            Self::Elf(namespace) => namespace.paths.clone(),
        }
    }

    pub fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Darwin(namespace) => namespace.providers.encode(&namespace.bindings, e),
            Self::Elf(namespace) => namespace.encode(e),
        }
    }

    pub fn dump(&self) -> String {
        match self {
            Self::Darwin(namespace) => namespace.providers.dump(&namespace.bindings),
            Self::Elf(namespace) => namespace.dump(),
        }
    }
}
