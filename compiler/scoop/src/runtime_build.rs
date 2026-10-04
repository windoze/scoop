//! Runtime compilation is orchestration, separate from single-Cone compilation.
use std::path::{Path, PathBuf};

use fs4::FileExt;
use scoop_linker::{RuntimeBuildConfiguration, RuntimeObjectSet};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::{Digest256, encode};

mod compile;
mod dependencies;
mod inputs;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default)]
pub enum RuntimeOptimization {
    None,
    #[default]
    Optimized,
}

pub struct RuntimeBuildRequest<'a> {
    pub target: &'a ResolvedTargetProfile,
    pub runtime_root: &'a Path,
    pub cache_root: &'a Path,
    pub optimization: RuntimeOptimization,
}

#[derive(Debug)]
pub struct RuntimeBuildOutput {
    objects: RuntimeObjectSet,
    index: PathBuf,
    cache_hit: bool,
    input_paths: Vec<PathBuf>,
}

impl RuntimeBuildOutput {
    pub fn objects(&self) -> &RuntimeObjectSet {
        &self.objects
    }
    pub fn into_objects(self) -> RuntimeObjectSet {
        self.objects
    }
    pub fn index(&self) -> &Path {
        &self.index
    }
    pub fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    pub fn input_paths(&self) -> &[PathBuf] {
        &self.input_paths
    }
}

#[derive(Debug)]
pub struct RuntimeBuildError(pub String);
impl std::fmt::Display for RuntimeBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RuntimeBuildError {}
fn error(value: impl std::fmt::Display) -> RuntimeBuildError {
    RuntimeBuildError(format!("runtime build: {value}"))
}

pub fn build_runtime(
    request: RuntimeBuildRequest<'_>,
) -> Result<RuntimeBuildOutput, RuntimeBuildError> {
    let inputs = inputs::Inputs::read(&request)?;
    let namespace = request.cache_root.join("runtime-v1");
    let locks = namespace.join(".locks");
    std::fs::create_dir_all(&locks).map_err(error)?;
    let lock = crate::cache::open_build_lock(&locks.join(format!("{}.lock", inputs.base_key)))
        .map_err(error)?;
    FileExt::lock(&lock).map_err(error)?;
    let entry = namespace.join(inputs.base_key.to_string());
    if let Some((objects, dependencies)) = cached_objects(&entry, request.target, &inputs) {
        return Ok(RuntimeBuildOutput {
            objects,
            index: entry.join("index.cbor"),
            cache_hit: true,
            input_paths: input_paths(&request, &inputs, &dependencies),
        });
    }
    let staging = tempfile::Builder::new()
        .prefix(".runtime-")
        .tempdir_in(&namespace)
        .map_err(error)?;
    let (bytes, dependencies) = compile::compile(&request, &inputs, staging.path())?;
    let input_key = dependencies.input_key(inputs.base_key)?;
    let configuration = RuntimeBuildConfiguration {
        input_key,
        compiler_digest: inputs.compiler_digest,
        flags: inputs.flags.clone(),
    };
    let objects = RuntimeObjectSet::from_objects(
        request.target.lir_target(),
        request.target.c_bridge_toolchain().profile(),
        configuration,
        bytes,
    )
    .map_err(error)?;
    let output = staging.path().join("objects");
    objects.write_index(&output).map_err(error)?;
    std::fs::write(
        output.join("dependencies.cbor"),
        encode(&dependencies).map_err(error)?,
    )
    .map_err(error)?;
    crate::cache::sync_cache_directory(&output).map_err(error)?;
    if entry.exists() {
        std::fs::remove_dir_all(&entry).map_err(error)?;
    }
    std::fs::rename(&output, &entry).map_err(error)?;
    crate::cache::sync_cache_directory(&namespace).map_err(error)?;
    Ok(RuntimeBuildOutput {
        objects,
        index: entry.join("index.cbor"),
        cache_hit: false,
        input_paths: input_paths(&request, &inputs, &dependencies),
    })
}

fn cached_objects(
    entry: &Path,
    target: &ResolvedTargetProfile,
    inputs: &inputs::Inputs,
) -> Option<(RuntimeObjectSet, dependencies::Dependencies)> {
    let bytes = std::fs::read(entry.join("dependencies.cbor")).ok()?;
    let dependencies: dependencies::Dependencies = scoop_wire::decode_canonical(&bytes).ok()?;
    if !dependencies.is_current() {
        return None;
    }
    let objects = RuntimeObjectSet::read_index(
        &entry.join("index.cbor"),
        target.lir_target(),
        target.c_bridge_toolchain().profile(),
    )
    .ok()?;
    (objects.configuration().input_key == dependencies.input_key(inputs.base_key).ok()?)
        .then_some((objects, dependencies))
}

fn input_paths(
    request: &RuntimeBuildRequest<'_>,
    inputs: &inputs::Inputs,
    dependencies: &dependencies::Dependencies,
) -> Vec<PathBuf> {
    inputs
        .files
        .keys()
        .map(|path| request.runtime_root.join(path))
        .chain(dependencies.input_paths())
        .collect()
}
