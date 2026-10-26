//! Runtime compilation is orchestration, separate from single-Cone compilation.
use std::path::{Path, PathBuf};

use fs4::FileExt;
use scoop_linker::{RuntimeBuildConfiguration, RuntimeObjectSet};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::{Digest256, encode};

mod compile;
mod dependencies;
mod inputs;
mod record;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default)]
pub enum RuntimeOptimization {
    None,
    #[default]
    Optimized,
}

impl RuntimeOptimization {
    fn c_flags(self) -> &'static [&'static str] {
        match self {
            Self::None => &["-O0"],
            Self::Optimized => &["-O2", "-DNDEBUG"],
        }
    }
}

pub struct RuntimeBuildRequest<'a> {
    pub target: &'a ResolvedTargetProfile,
    pub runtime_root: &'a Path,
    pub vendor_root: &'a Path,
    pub cache_root: &'a Path,
    pub optimization: RuntimeOptimization,
    pub enable_gc_stress: bool,
    pub enable_startup_stats: bool,
    pub unwind_prefix: Option<&'a Path>,
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
    let namespace = request.cache_root.join("runtime-v7");
    let entry = namespace.join(format!("{}.json", inputs.base_key));
    if let Some(output) = record::load(&entry, request.target)? {
        return Ok(output);
    }
    std::fs::create_dir_all(&namespace).map_err(error)?;
    let lock = crate::cache::open_build_lock(&namespace.join(format!("{}.lock", inputs.base_key)))
        .map_err(error)?;
    FileExt::lock(&lock).map_err(error)?;
    if let Some(output) = record::load(&entry, request.target)? {
        return Ok(output);
    }
    let staging = tempfile::Builder::new()
        .prefix("objects-")
        .tempdir_in(&namespace)
        .map_err(error)?;
    let object_directory = staging.path().join("objects");
    std::fs::create_dir(&object_directory).map_err(error)?;
    let (paths, dependencies) = compile::compile(&request, &inputs, &object_directory)?;
    let configuration = RuntimeBuildConfiguration {
        input_key: dependencies.input_key(inputs.base_key)?,
        compiler_inputs: inputs.compiler_inputs,
        flags: inputs.flags,
        enable_gc_stress: request.enable_gc_stress,
        enable_startup_stats: request.enable_startup_stats,
    };
    let objects = RuntimeObjectSet::from_paths(
        request.target.lir_target(),
        request.target.c_bridge_toolchain().profile(),
        configuration,
        paths,
    )
    .map_err(error)?;
    let index = objects.write_index(staging.path()).map_err(error)?;
    let input_paths = dependencies.input_paths()?;
    let record = record::Record {
        dependencies,
        index: scoop_protocol::HostPathCarrier::from_path(&index).map_err(error)?,
    };
    let _ = staging.keep();
    record::save(&entry, &record)?;
    Ok(RuntimeBuildOutput {
        objects,
        index,
        cache_hit: false,
        input_paths,
    })
}
