//! A native runtime archive, with an index of its already inspected members.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ar_archive_writer::{DEFAULT_OBJECT_READER, NewArchiveMember};

use scoop_lir::{
    CBridgeToolchainProfileV1, CompilerNativeContractV1, LirTargetProfile, RuntimeAbiContract,
    RuntimeAbiSymbolV1, RuntimeSymbolContractRegistryV1,
};
use scoop_wire::{Digest256, domain_separated_cbor_hash, encode, sha256};

use crate::{LinkError, NativeObjectInfo, NativeSymbolKind, error};

#[cfg(all(test, target_arch = "x86_64", any(target_os = "linux", windows)))]
mod common_tests;
mod deployment;
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
    pub compiler_inputs: Digest256,
    pub flags: Vec<String>,
    pub enable_gc_stress: bool,
    pub enable_startup_stats: bool,
    pub enable_profiling: bool,
}

#[derive(Clone, Debug)]
pub struct RuntimeObject {
    id: RuntimeObjectId,
    digest: Digest256,
    info: NativeObjectInfo,
}
impl RuntimeObject {
    pub fn id(&self) -> RuntimeObjectId {
        self.id
    }
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
    pub fn info(&self) -> &NativeObjectInfo {
        &self.info
    }
}

#[derive(Debug)]
pub struct RuntimeObjectSet {
    target: LirTargetProfile,
    toolchain: Vec<u8>,
    deployment: deployment::RuntimeDeployment,
    configuration: RuntimeBuildConfiguration,
    objects: Vec<RuntimeObject>,
    merged: NativeObjectInfo,
    fingerprint: RuntimeArtifactFingerprint,
    input_paths: Vec<PathBuf>,
}

impl RuntimeObjectSet {
    pub fn from_paths(
        target: LirTargetProfile,
        toolchain: &CBridgeToolchainProfileV1,
        configuration: RuntimeBuildConfiguration,
        objects: Vec<PathBuf>,
        directory: &Path,
        profile_runtime: Option<&Path>,
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
        for path in objects {
            let bytes = crate::native_archive::prepare_object(
                target,
                std::borrow::Cow::Owned(std::fs::read(&path).map_err(error)?),
            )?;
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
            inputs.push((RuntimeObject { id, digest, info }, bytes));
        }
        inputs.sort_by_key(|(object, _)| object.id);
        let (objects, bytes): (Vec<_>, Vec<_>) = inputs.into_iter().unzip();
        let directory = std::path::absolute(directory).map_err(error)?;
        let (kind, extension) = crate::native_archive::format(target);
        let mut paths = vec![
            directory.join("index.cbor"),
            directory.join(format!("runtime.{extension}")),
        ];
        if configuration.enable_profiling != profile_runtime.is_some() {
            return Err(error(
                "profiling runtime requires its compiler-rt profile archive",
            ));
        }
        if profile_runtime.is_some() {
            paths.push(directory.join("profile.a"));
        }
        let runtime = Self::assemble(target, toolchain, configuration, objects, paths)?;
        let members: Vec<_> = bytes
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                NewArchiveMember::new(bytes.as_ref(), &DEFAULT_OBJECT_READER, format!("{index}.o"))
            })
            .collect();
        std::fs::create_dir_all(directory).map_err(error)?;
        if let Some(source) = profile_runtime {
            std::fs::copy(
                source,
                runtime.profile_runtime().expect("profile archive output"),
            )
            .map_err(error)?;
        }
        crate::native_archive::write(runtime.archive_path(), kind, &members)?;
        runtime.write_index()?;
        Ok(runtime)
    }

    fn assemble(
        target: LirTargetProfile,
        toolchain: &CBridgeToolchainProfileV1,
        configuration: RuntimeBuildConfiguration,
        objects: Vec<RuntimeObject>,
        input_paths: Vec<PathBuf>,
    ) -> Result<Self, LinkError> {
        let deployment = deployment::RuntimeDeployment::for_toolchain(toolchain);
        let toolchain = encode(toolchain.contract()).map_err(error)?;
        if objects.is_empty() || objects.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(error(
                "runtime object collection is empty or has duplicate object IDs",
            ));
        }
        let mut merged = NativeObjectInfo {
            definitions: BTreeMap::new(),
            requirements: BTreeSet::new(),
        };
        let definition = scoop_target_data::definition(target.id())
            .filter(|definition| definition.selection.target() == target)
            .ok_or_else(|| error("runtime object collection requires a registered target"))?;
        let normalization = target.contract().native_symbol_normalization();
        let native_name = |name| normalization.compiler_generated_object_symbol(name);
        let program_entry = native_name(if definition.os == scoop_target_data::TargetOs::Windows {
            "wmain"
        } else {
            "main"
        });
        let string_descriptor = native_name("scoop_td_String");
        let generated_prefix = native_name("scoop$");
        for object in &objects {
            for (symbol, definition) in &object.info.definitions {
                if symbol == &program_entry
                    || symbol == &string_descriptor
                    || symbol.starts_with(&generated_prefix)
                {
                    return Err(error(format!(
                        "runtime object {} defines program-owned symbol {symbol}",
                        object.id
                    )));
                }
                match merged.definitions.entry(symbol.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(*definition);
                    }
                    std::collections::btree_map::Entry::Occupied(entry)
                        if target.native_object_format()
                            == scoop_lir::NativeObjectFormat::Coff64
                            && definition.binding == crate::NativeSymbolBinding::Weak
                            && entry.get() == definition =>
                    {
                        // CRT inline helpers/refptrs coalesce in the real PE
                        // link. Every ABI export must still be strong below.
                        continue;
                    }
                    _ => {
                        return Err(error(format!(
                            "runtime has multiple definitions of {symbol}"
                        )));
                    }
                }
            }
            merged
                .requirements
                .extend(object.info.requirements.iter().cloned());
        }
        check_exports(target, &merged)?;
        let fingerprint = RuntimeArtifactFingerprint(
            domain_separated_cbor_hash(
                "scoop-runtime-artifact-v2",
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
            deployment,
            configuration,
            objects,
            merged,
            fingerprint,
            input_paths,
        })
    }

    pub fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub fn input_paths(&self) -> &[std::path::PathBuf] {
        &self.input_paths
    }
    pub fn index_path(&self) -> &Path {
        &self.input_paths[0]
    }
    pub fn archive_path(&self) -> &Path {
        &self.input_paths[1]
    }
    pub fn profile_runtime(&self) -> Option<&Path> {
        self.input_paths.get(2).map(PathBuf::as_path)
    }

    pub fn require_profiling(&self, enabled: bool) -> Result<(), LinkError> {
        if self.configuration.enable_profiling != enabled {
            return Err(error(format!(
                "runtime profiling support is {}, but build instrumentation is {}",
                self.configuration.enable_profiling, enabled
            )));
        }
        Ok(())
    }

    pub fn configuration(&self) -> &RuntimeBuildConfiguration {
        &self.configuration
    }

    pub fn require_gc_stress(&self, enabled: bool) -> Result<(), LinkError> {
        if self.configuration.enable_gc_stress != enabled {
            return Err(error(format!(
                "runtime GC stress support is {}, but --enable-gc-stress is {}",
                self.configuration.enable_gc_stress, enabled
            )));
        }
        Ok(())
    }
    pub fn require_startup_stats(&self, enabled: bool) -> Result<(), LinkError> {
        if self.configuration.enable_startup_stats != enabled {
            return Err(error(format!(
                "runtime startup statistics support is {}, but --enable-startup-stats is {}",
                self.configuration.enable_startup_stats, enabled
            )));
        }
        Ok(())
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
        if contract.symbol() == RuntimeAbiSymbolV1::CoreStringTypeDescriptor {
            continue;
        }
        let expected = match contract.symbol().machine_contract() {
            CompilerNativeContractV1::Data {
                thread_local: true, ..
            } => NativeSymbolKind::ThreadLocal,
            CompilerNativeContractV1::Data {
                thread_local: false,
                ..
            } => NativeSymbolKind::Data,
            CompilerNativeContractV1::Function { .. } => NativeSymbolKind::Function,
        };
        if !merged.definitions.get(&symbol).is_some_and(|definition| {
            definition.kind == expected && definition.binding == crate::NativeSymbolBinding::Strong
        }) {
            return Err(error(format!(
                "runtime is missing the {expected:?} ABI definition {symbol}"
            )));
        }
    }
    let startup = target
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol("scoop_rt_run_program");
    if !merged.definitions.get(&startup).is_some_and(|definition| {
        definition.kind == NativeSymbolKind::Function
            && definition.binding == crate::NativeSymbolBinding::Strong
    }) {
        return Err(error(format!(
            "runtime is missing the program startup entry {startup}"
        )));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_tests;
