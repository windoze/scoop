//! Native preprocessing and ordinary reusable object builds.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ValidatedCBridgeToolchainInvocation;
use scoop_lir::{OptimizationMode, TargetProfileId};
use scoop_manifest::{ConeRelativePath, LoadedConeManifest, NativeConfig, NativeSourceLanguage};
use scoop_wire::{Digest256, sha256};

use crate::ToolchainError;

mod command;
mod includes;
pub use includes::strip_native_include_observations;
mod objects;
pub use objects::{PreparedNativeObjects, prepare_native_objects, selected_native_object_sources};
mod paths;
mod prepare;
mod toolchain;
pub use toolchain::NativeToolchain;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSourceInput {
    path: ConeRelativePath,
    language: NativeSourceLanguage,
    preprocessed: Arc<[u8]>,
    digest: Digest256,
}

impl NativeSourceInput {
    pub fn new(
        path: ConeRelativePath,
        language: NativeSourceLanguage,
        preprocessed: Arc<[u8]>,
    ) -> Self {
        let digest = sha256(&preprocessed);
        Self {
            path,
            language,
            preprocessed,
            digest,
        }
    }

    pub fn path(&self) -> &ConeRelativePath {
        &self.path
    }

    pub fn language(&self) -> NativeSourceLanguage {
        self.language
    }

    pub fn preprocessed(&self) -> &[u8] {
        &self.preprocessed
    }
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreparedNativeInputs {
    units: Vec<NativeSourceInput>,
    observed_paths: BTreeSet<PathBuf>,
}

impl PreparedNativeInputs {
    pub fn units(&self) -> &[NativeSourceInput] {
        &self.units
    }
}

pub fn prepare_native_inputs(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    compiler: &ValidatedCBridgeToolchainInvocation,
    optimization: OptimizationMode,
    sysroot: &Path,
) -> Result<PreparedNativeInputs, ToolchainError> {
    let compiler = NativeToolchain::resolve(compiler, manifest)?;
    prepare::prepare(manifest, target, &compiler, optimization, sysroot)
}

pub fn compile_native_source(
    input: &NativeSourceInput,
    config: &NativeConfig,
    compiler: &NativeToolchain,
    optimization: OptimizationMode,
    output: &Path,
) -> Result<(), ToolchainError> {
    command::compile(input, config, compiler, optimization, output)
}

fn error(error: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native compilation: {error}"))
}
