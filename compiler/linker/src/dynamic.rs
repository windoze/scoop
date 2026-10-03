//! Native dynamic interfaces and the provider chosen for each machine import.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_toolchain::{NativeExport, SystemExportKind, ValidatedFinalLinkProfile};
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash};

use crate::native_input::{NativeInputId, NativeInputs};
use crate::{LinkError, NativeSymbolDefinition, error};

mod exports;
mod graph;
mod locate;
mod plan;
mod read;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NativeDynamicProviderId(Digest256);
impl std::fmt::Display for NativeDynamicProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl WireEncode for NativeDynamicProviderId {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(e)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ExportStorage {
    InterfaceOnly,
    Definition(NativeSymbolDefinition),
}

#[derive(Clone, Debug)]
pub(crate) enum DynamicExport {
    Symbol {
        interface: NativeExport,
        storage: ExportStorage,
    },
    Reexport {
        dependency: usize,
        imported: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct LoadDependency {
    pub name: String,
    pub reexport: bool,
    pub compatibility_version: u32,
}

pub(crate) struct DynamicProvider {
    pub id: NativeDynamicProviderId,
    pub input: NativeInputId,
    pub install_name: String,
    pub current_version: u32,
    pub compatibility_version: u32,
    pub exports: BTreeMap<String, DynamicExport>,
    pub dependencies: Vec<LoadDependency>,
    pub rpaths: Vec<String>,
    pub locator: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct DynamicBinding {
    pub owner: NativeDynamicProviderId,
    pub source: NativeDynamicProviderId,
    pub source_symbol: String,
    pub interface: NativeExport,
    pub storage: ExportStorage,
}

#[derive(Default)]
pub(crate) struct DynamicInputs {
    pub providers: BTreeMap<NativeDynamicProviderId, Arc<DynamicProvider>>,
    pub roots: BTreeSet<NativeDynamicProviderId>,
    pub edges: BTreeMap<(NativeDynamicProviderId, usize), NativeDynamicProviderId>,
    pub rpaths: BTreeSet<String>,
    pub stubs: BTreeMap<NativeDynamicProviderId, Vec<u8>>,
}

struct ProviderKey<'a> {
    input: NativeInputId,
    name: &'a str,
    profile: &'a ValidatedFinalLinkProfile,
}
impl WireEncode for ProviderKey<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(3)?;
        self.profile.target().wire_id().encode(e)?;
        self.input.encode(e)?;
        e.text(self.name)
    }
}

impl DynamicProvider {
    fn id(
        input: NativeInputId,
        name: &str,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<NativeDynamicProviderId, LinkError> {
        Ok(NativeDynamicProviderId(
            domain_separated_cbor_hash(
                "scoop-native-dynamic-provider-v1",
                &ProviderKey {
                    input,
                    name,
                    profile,
                },
            )
            .map_err(error)?,
        ))
    }
}

pub(crate) use read::read;
