use scoop_process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use scoop_protocol::{
    ScoopcMachineCapabilityV1, ScoopcProtocolCapabilityV1, decode_machine_capability_frame,
};
use scoop_wire::{Digest256, Encoder, WireEncode};

use crate::{ArtifactCacheRoot, PairedScoopcLocator};

mod error;
mod record;

pub use error::PairedCompilerError;
use record::{CompilerInput, CompilerRecord};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairedCompilerFingerprintV1 {
    input_fingerprint: Digest256,
    toolchain_distribution_id: Digest256,
    compiler_build_identity: Digest256,
}

impl PairedCompilerFingerprintV1 {
    pub(crate) const fn from_parts(
        input_fingerprint: Digest256,
        toolchain_distribution_id: Digest256,
        compiler_build_identity: Digest256,
    ) -> Self {
        Self {
            input_fingerprint,
            toolchain_distribution_id,
            compiler_build_identity,
        }
    }

    pub const fn input_fingerprint(self) -> Digest256 {
        self.input_fingerprint
    }

    pub const fn toolchain_distribution_id(self) -> Digest256 {
        self.toolchain_distribution_id
    }

    pub const fn compiler_build_identity(self) -> Digest256 {
        self.compiler_build_identity
    }
}

impl WireEncode for PairedCompilerFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.input_fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.toolchain_distribution_id.encode(encoder)?;
        encoder.field(3)?;
        self.compiler_build_identity.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPairedScoopc {
    executable: PathBuf,
    fingerprint: PairedCompilerFingerprintV1,
    protocol: ScoopcProtocolCapabilityV1,
}

impl ResolvedPairedScoopc {
    pub fn resolve(
        locator: &PairedScoopcLocator,
        cache_root: &ArtifactCacheRoot,
    ) -> Result<Self, PairedCompilerError> {
        let executable =
            locator
                .as_path()
                .canonicalize()
                .map_err(|source| PairedCompilerError::Inspect {
                    path: locator.as_path().to_path_buf(),
                    source,
                })?;
        let input = CompilerInput::read(&executable)?;
        let expected = scoop_toolchain::paired_compiler_machine_capability()
            .map_err(PairedCompilerError::ExpectedCapability)?;
        let capability =
            CompilerRecord::load_or_probe(cache_root.as_path(), &input, &executable, expected)?;
        let fingerprint = PairedCompilerFingerprintV1::from_parts(
            input.fingerprint()?,
            capability.toolchain_distribution_id(),
            capability.compiler_build_identity(),
        );
        Ok(Self {
            executable,
            fingerprint,
            protocol: capability.protocol(),
        })
    }

    pub fn executable_path(&self) -> &Path {
        &self.executable
    }

    pub const fn fingerprint(&self) -> PairedCompilerFingerprintV1 {
        self.fingerprint
    }

    pub const fn protocol(&self) -> ScoopcProtocolCapabilityV1 {
        self.protocol
    }
}

fn probe_capability(
    executable: &Path,
    expected: ScoopcMachineCapabilityV1,
) -> Result<(ScoopcMachineCapabilityV1, Vec<u8>), PairedCompilerError> {
    let output = invoke_capability(executable)?;
    if !output.status.success() {
        return Err(PairedCompilerError::Exit {
            status: output.status,
            stderr: output.stderr,
        });
    }
    if !output.stderr.is_empty() {
        return Err(PairedCompilerError::UnexpectedStderr(output.stderr));
    }
    let capability = decode_machine_capability_frame(&output.stdout)
        .map_err(PairedCompilerError::CapabilityFrame)?;
    if capability != expected {
        return Err(PairedCompilerError::CapabilityMismatch {
            expected: Box::new(expected),
            actual: Box::new(capability),
        });
    }
    Ok((capability, output.stdout))
}

fn invoke_capability(executable: &Path) -> Result<Output, PairedCompilerError> {
    let working_directory = executable
        .parent()
        .ok_or_else(|| PairedCompilerError::MissingExecutableParent(executable.to_path_buf()))?;
    Command::new(executable)
        .arg("__machine-capability")
        .env_clear()
        .current_dir(working_directory)
        .scoop_output()
        .map_err(|source| PairedCompilerError::Spawn {
            path: executable.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests;
