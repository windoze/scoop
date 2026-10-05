//! An ordinary object collection, with a local index for cache/process handoff.
use std::collections::{BTreeMap, BTreeSet};

use scoop_lir::{
    CBridgeToolchainProfileV1, LirTargetProfile, RuntimeAbiContract, RuntimeAbiSymbolV1,
    RuntimeSymbolContractRegistryV1,
};
use scoop_wire::{Digest256, domain_separated_cbor_hash, encode, sha256};

use crate::{LinkError, NativeObjectInfo, NativeSymbolKind, error};

mod fingerprint;
mod index;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeObjectId(Digest256);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactFingerprint(Digest256);

impl RuntimeObjectId {
    pub fn digest(self) -> Digest256 {
        self.0
    }
}
impl RuntimeArtifactFingerprint {
    pub fn digest(self) -> Digest256 {
        self.0
    }
}
impl std::fmt::Display for RuntimeObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::fmt::Display for RuntimeArtifactFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBuildConfiguration {
    pub input_key: Digest256,
    pub compiler_digest: Digest256,
    pub flags: Vec<String>,
}

#[derive(Debug)]
pub struct RuntimeObject {
    id: RuntimeObjectId,
    digest: Digest256,
    bytes: Vec<u8>,
    info: NativeObjectInfo,
}
impl RuntimeObject {
    pub fn id(&self) -> RuntimeObjectId {
        self.id
    }
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn info(&self) -> &NativeObjectInfo {
        &self.info
    }
}

#[derive(Debug)]
pub struct RuntimeObjectSet {
    target: LirTargetProfile,
    toolchain: Vec<u8>,
    configuration: RuntimeBuildConfiguration,
    objects: Vec<RuntimeObject>,
    merged: NativeObjectInfo,
    fingerprint: RuntimeArtifactFingerprint,
    input_paths: Vec<std::path::PathBuf>,
}

impl RuntimeObjectSet {
    pub fn from_objects(
        target: LirTargetProfile,
        toolchain: &CBridgeToolchainProfileV1,
        configuration: RuntimeBuildConfiguration,
        objects: Vec<Vec<u8>>,
    ) -> Result<Self, LinkError> {
        if objects.is_empty() {
            return Err(error("runtime requires a nonempty object collection"));
        }
        if toolchain.contract().target() != &target.wire_id() {
            return Err(error(
                "runtime C toolchain and selected target do not match",
            ));
        }
        let mut inputs = Vec::with_capacity(objects.len());
        for bytes in objects {
            let info = NativeObjectInfo::read_with_toolchain(&bytes, toolchain)?;
            let digest = sha256(&bytes);
            let id = RuntimeObjectId(
                domain_separated_cbor_hash(
                    "scoop-runtime-object-v1",
                    &fingerprint::ObjectKey {
                        digest,
                        info: &info,
                    },
                )
                .map_err(error)?,
            );
            inputs.push(RuntimeObject {
                id,
                digest,
                bytes,
                info,
            });
        }
        Self::assemble(
            target,
            encode(toolchain.contract()).map_err(error)?,
            configuration,
            inputs,
        )
    }

    fn assemble(
        target: LirTargetProfile,
        toolchain: Vec<u8>,
        configuration: RuntimeBuildConfiguration,
        mut objects: Vec<RuntimeObject>,
    ) -> Result<Self, LinkError> {
        objects.sort_by_key(|object| object.id);
        if objects.is_empty() || objects.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(error(
                "runtime object collection is empty or has duplicate object IDs",
            ));
        }
        let mut merged = NativeObjectInfo {
            definitions: BTreeMap::new(),
            requirements: BTreeSet::new(),
        };
        let normalization = target.contract().native_symbol_normalization();
        let native_name = |name| normalization.compiler_generated_object_symbol(name);
        let main = native_name("main");
        let string_descriptor = native_name("scoop_td_String");
        let generated_prefix = native_name("scoop$");
        for object in &objects {
            for (symbol, definition) in &object.info.definitions {
                if symbol == &main
                    || symbol == &string_descriptor
                    || symbol.starts_with(&generated_prefix)
                {
                    return Err(error(format!(
                        "runtime object {} defines program-owned symbol {symbol}",
                        object.id
                    )));
                }
                if merged
                    .definitions
                    .insert(symbol.clone(), *definition)
                    .is_some()
                {
                    return Err(error(format!(
                        "runtime has multiple definitions of {symbol}"
                    )));
                }
            }
            merged
                .requirements
                .extend(object.info.requirements.iter().cloned());
        }
        check_exports(target, &merged)?;
        let fingerprint = RuntimeArtifactFingerprint(
            domain_separated_cbor_hash(
                "scoop-runtime-artifact-v1",
                &fingerprint::ArtifactKey {
                    target,
                    toolchain: &toolchain,
                    configuration: &configuration,
                    objects: &objects,
                    merged: &merged,
                },
            )
            .map_err(error)?,
        );
        Ok(Self {
            target,
            toolchain,
            configuration,
            objects,
            merged,
            fingerprint,
            input_paths: Vec::new(),
        })
    }

    pub fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub fn input_paths(&self) -> &[std::path::PathBuf] {
        &self.input_paths
    }
    pub fn configuration(&self) -> &RuntimeBuildConfiguration {
        &self.configuration
    }
    pub fn objects(&self) -> &[RuntimeObject] {
        &self.objects
    }
    pub fn symbols(&self) -> &NativeObjectInfo {
        &self.merged
    }
    pub fn fingerprint(&self) -> RuntimeArtifactFingerprint {
        self.fingerprint
    }
}

fn check_exports(target: LirTargetProfile, merged: &NativeObjectInfo) -> Result<(), LinkError> {
    let registry = RuntimeSymbolContractRegistryV1::current(target).map_err(error)?;
    for contract in registry.contracts() {
        let symbol = String::from_utf8(contract.object_symbol(target)).map_err(error)?;
        let expected = match contract.symbol() {
            RuntimeAbiSymbolV1::CoreStringTypeDescriptor => continue,
            RuntimeAbiSymbolV1::AllocationContext => NativeSymbolKind::ThreadLocal,
            RuntimeAbiSymbolV1::CardTable => NativeSymbolKind::Data,
            _ => NativeSymbolKind::Function,
        };
        if !merged
            .definitions
            .get(&symbol)
            .is_some_and(|definition| definition.kind == expected && !definition.weak)
        {
            return Err(error(format!(
                "runtime is missing the {expected:?} ABI definition {symbol}"
            )));
        }
    }
    let startup = target
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol("scoop_rt_run_program");
    if !merged
        .definitions
        .get(&startup)
        .is_some_and(|definition| definition.kind == NativeSymbolKind::Function && !definition.weak)
    {
        return Err(error(format!(
            "runtime is missing the program startup entry {startup}"
        )));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
