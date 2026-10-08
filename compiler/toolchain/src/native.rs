//! Native translation units prepared before the outer Cone cache lookup.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedCBridgeToolchainInvocation};
use scoop_manifest::{ConeRelativePath, LoadedConeManifest, NativeConfig};
use scoop_wire::{Digest256, Encoder, WireEncode, sha256};

use crate::ToolchainError;

mod command;
mod paths;
mod prepare;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSourceInput {
    path: ConeRelativePath,
    preprocessed: Arc<[u8]>,
    digest: Digest256,
}

impl NativeSourceInput {
    pub fn new(path: ConeRelativePath, preprocessed: Arc<[u8]>) -> Self {
        let digest = sha256(&preprocessed);
        Self {
            path,
            preprocessed,
            digest,
        }
    }

    pub fn path(&self) -> &ConeRelativePath {
        &self.path
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
    dependencies: BTreeMap<String, Digest256>,
    flags: Vec<String>,
    libraries: Vec<scoop_lir::CanonicalNativeLibraryRequirementV1>,
}

impl PreparedNativeInputs {
    pub fn units(&self) -> &[NativeSourceInput] {
        &self.units
    }
}

impl WireEncode for PreparedNativeInputs {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(4)?;
        encoder.array(self.units.len() as u64)?;
        for unit in &self.units {
            encoder.array(2)?;
            encoder.text(unit.path.as_str())?;
            unit.digest().encode(encoder)?;
        }
        encoder.array(self.dependencies.len() as u64)?;
        for (path, digest) in &self.dependencies {
            encoder.array(2)?;
            encoder.text(path)?;
            digest.encode(encoder)?;
        }
        encoder.array(self.flags.len() as u64)?;
        for flag in &self.flags {
            encoder.text(flag)?;
        }
        encoder.array(self.libraries.len() as u64)?;
        for library in &self.libraries {
            library.encode(encoder)?;
        }
        Ok(())
    }
}

pub fn prepare_native_inputs(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    compiler: &ValidatedCBridgeToolchainInvocation,
    optimization: OptimizationMode,
    public_include: &Path,
) -> Result<PreparedNativeInputs, ToolchainError> {
    prepare::prepare(manifest, target, compiler, optimization, public_include)
}

pub fn compile_native_source(
    input: &NativeSourceInput,
    config: &NativeConfig,
    compiler: &ValidatedCBridgeToolchainInvocation,
    optimization: OptimizationMode,
    output: &Path,
) -> Result<(), ToolchainError> {
    command::compile(input, config, compiler, optimization, output)
}

fn error(error: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native compilation: {error}"))
}
