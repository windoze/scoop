use std::fmt;
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use scoop_protocol::{
    ProtocolReadError, ScoopcMachineCapabilityV1, ScoopcProtocolCapabilityV1,
    decode_machine_capability_frame,
};
use scoop_wire::{Digest256, Encoder, HashError, WireEncode};

use crate::{ImmutableInputSnapshot, PairedScoopcLocator, SnapshotFileError};

const MAX_PAIRED_COMPILER_BYTES: u64 = 1_073_741_824;
const MAX_CAPABILITY_STDOUT_BYTES: u64 = 4_096;
const MAX_CAPABILITY_STDERR_BYTES: u64 = 65_536;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairedCompilerFingerprintV1 {
    executable_sha256: Digest256,
    toolchain_distribution_id: Digest256,
    compiler_build_identity: Digest256,
}

impl PairedCompilerFingerprintV1 {
    pub(crate) const fn from_parts(
        executable_sha256: Digest256,
        toolchain_distribution_id: Digest256,
        compiler_build_identity: Digest256,
    ) -> Self {
        Self {
            executable_sha256,
            toolchain_distribution_id,
            compiler_build_identity,
        }
    }

    pub const fn executable_sha256(self) -> Digest256 {
        self.executable_sha256
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
        self.executable_sha256.encode(encoder)?;
        encoder.field(2)?;
        self.toolchain_distribution_id.encode(encoder)?;
        encoder.field(3)?;
        self.compiler_build_identity.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPairedScoopc {
    executable: ImmutableInputSnapshot,
    fingerprint: PairedCompilerFingerprintV1,
    protocol: ScoopcProtocolCapabilityV1,
}

impl ResolvedPairedScoopc {
    pub fn resolve(locator: &PairedScoopcLocator) -> Result<Self, PairedCompilerError> {
        let executable =
            ImmutableInputSnapshot::capture(locator.as_path(), MAX_PAIRED_COMPILER_BYTES)
                .map_err(PairedCompilerError::Snapshot)?;
        ensure_executable(executable.resolved_path())?;
        let output = invoke_capability(executable.resolved_path())?;
        let after = ImmutableInputSnapshot::capture(locator.as_path(), MAX_PAIRED_COMPILER_BYTES)
            .map_err(PairedCompilerError::Snapshot)?;
        if executable.resolved_path() != after.resolved_path()
            || executable.digest() != after.digest()
        {
            return Err(PairedCompilerError::Changed {
                before: executable.digest(),
                after: after.digest(),
            });
        }
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
        let expected = scoop_toolchain::paired_compiler_machine_capability()
            .map_err(PairedCompilerError::ExpectedCapability)?;
        if capability != expected {
            return Err(PairedCompilerError::CapabilityMismatch {
                expected: Box::new(expected),
                actual: Box::new(capability),
            });
        }
        let fingerprint = PairedCompilerFingerprintV1::from_parts(
            executable.digest(),
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
        self.executable.resolved_path()
    }

    pub const fn fingerprint(&self) -> PairedCompilerFingerprintV1 {
        self.fingerprint
    }

    pub const fn protocol(&self) -> ScoopcProtocolCapabilityV1 {
        self.protocol
    }

    pub const fn executable_snapshot(&self) -> &ImmutableInputSnapshot {
        &self.executable
    }

    pub(crate) fn revalidate_executable(&self) -> Result<(), PairedCompilerError> {
        let after = ImmutableInputSnapshot::capture(
            self.executable.source_locator(),
            MAX_PAIRED_COMPILER_BYTES,
        )
        .map_err(PairedCompilerError::Snapshot)?;
        if self.executable.resolved_path() != after.resolved_path()
            || self.executable.digest() != after.digest()
        {
            return Err(PairedCompilerError::Changed {
                before: self.executable.digest(),
                after: after.digest(),
            });
        }
        ensure_executable(after.resolved_path())
    }
}

fn ensure_executable(path: &Path) -> Result<(), PairedCompilerError> {
    let metadata = std::fs::metadata(path).map_err(|source| PairedCompilerError::Inspect {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(PairedCompilerError::NotExecutable(path.to_path_buf()));
        }
    }
    Ok(())
}

struct CapabilityOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn invoke_capability(executable: &Path) -> Result<CapabilityOutput, PairedCompilerError> {
    let working_directory = executable
        .parent()
        .ok_or_else(|| PairedCompilerError::MissingExecutableParent(executable.to_path_buf()))?;
    let mut child = Command::new(executable)
        .arg("__machine-capability")
        .env_clear()
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| PairedCompilerError::Spawn {
            path: executable.to_path_buf(),
            source,
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or(PairedCompilerError::MissingPipe("stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(PairedCompilerError::MissingPipe("stderr"))?;
    let stdout_thread = std::thread::spawn(move || {
        read_bounded_stream(stdout, "stdout", MAX_CAPABILITY_STDOUT_BYTES)
    });
    let stderr_thread = std::thread::spawn(move || {
        read_bounded_stream(stderr, "stderr", MAX_CAPABILITY_STDERR_BYTES)
    });
    let status = child.wait().map_err(PairedCompilerError::Wait)?;
    let stdout = stdout_thread
        .join()
        .map_err(|_| PairedCompilerError::ReaderPanicked("stdout"))??;
    let stderr = stderr_thread
        .join()
        .map_err(|_| PairedCompilerError::ReaderPanicked("stderr"))??;
    Ok(CapabilityOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_bounded_stream(
    stream: impl Read,
    name: &'static str,
    limit: u64,
) -> Result<Vec<u8>, PairedCompilerError> {
    let read_limit = limit
        .checked_add(1)
        .ok_or(PairedCompilerError::StreamLengthOverflow(name))?;
    let mut bytes = Vec::new();
    stream
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| PairedCompilerError::ReadStream { name, source })?;
    let observed =
        u64::try_from(bytes.len()).map_err(|_| PairedCompilerError::StreamLengthOverflow(name))?;
    if observed > limit {
        return Err(PairedCompilerError::StreamTooLarge {
            name,
            limit,
            observed,
        });
    }
    Ok(bytes)
}

#[derive(Debug)]
pub enum PairedCompilerError {
    Snapshot(SnapshotFileError),
    Inspect {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    NotExecutable(std::path::PathBuf),
    MissingExecutableParent(std::path::PathBuf),
    Spawn {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    MissingPipe(&'static str),
    Wait(std::io::Error),
    ReadStream {
        name: &'static str,
        source: std::io::Error,
    },
    StreamLengthOverflow(&'static str),
    StreamTooLarge {
        name: &'static str,
        limit: u64,
        observed: u64,
    },
    ReaderPanicked(&'static str),
    Changed {
        before: Digest256,
        after: Digest256,
    },
    Exit {
        status: ExitStatus,
        stderr: Vec<u8>,
    },
    UnexpectedStderr(Vec<u8>),
    CapabilityFrame(ProtocolReadError),
    ExpectedCapability(HashError),
    CapabilityMismatch {
        expected: Box<ScoopcMachineCapabilityV1>,
        actual: Box<ScoopcMachineCapabilityV1>,
    },
}

impl fmt::Display for PairedCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot(error) => error.fmt(formatter),
            Self::Inspect { path, source } => {
                write!(
                    formatter,
                    "cannot inspect paired compiler {}: {source}",
                    path.display()
                )
            }
            Self::NotExecutable(path) => {
                write!(
                    formatter,
                    "paired compiler {} is not executable",
                    path.display()
                )
            }
            Self::MissingExecutableParent(path) => write!(
                formatter,
                "paired compiler {} has no parent directory",
                path.display()
            ),
            Self::Spawn { path, source } => write!(
                formatter,
                "cannot start paired compiler {} capability handshake: {source}",
                path.display()
            ),
            Self::MissingPipe(name) => {
                write!(
                    formatter,
                    "paired compiler capability {name} pipe is unavailable"
                )
            }
            Self::Wait(error) => write!(
                formatter,
                "cannot wait for paired compiler capability handshake: {error}"
            ),
            Self::ReadStream { name, source } => {
                write!(formatter, "cannot read paired compiler {name}: {source}")
            }
            Self::StreamLengthOverflow(name) => {
                write!(formatter, "paired compiler {name} length overflowed")
            }
            Self::StreamTooLarge {
                name,
                limit,
                observed,
            } => write!(
                formatter,
                "paired compiler {name} exceeds byte limit {limit}: observed {observed}"
            ),
            Self::ReaderPanicked(name) => {
                write!(formatter, "paired compiler {name} reader panicked")
            }
            Self::Changed { before, after } => write!(
                formatter,
                "paired compiler changed during capability handshake: {before} -> {after}"
            ),
            Self::Exit { status, stderr } => write!(
                formatter,
                "paired compiler capability handshake exited with {status}; stderr={:?}",
                String::from_utf8_lossy(stderr)
            ),
            Self::UnexpectedStderr(stderr) => write!(
                formatter,
                "paired compiler capability handshake wrote stderr: {:?}",
                String::from_utf8_lossy(stderr)
            ),
            Self::CapabilityFrame(error) => {
                write!(
                    formatter,
                    "invalid paired compiler capability frame: {error}"
                )
            }
            Self::ExpectedCapability(error) => write!(
                formatter,
                "cannot construct expected paired compiler capability: {error}"
            ),
            Self::CapabilityMismatch { expected, actual } => write!(
                formatter,
                "paired compiler capability mismatch: expected {expected:?}, found {actual:?}"
            ),
        }
    }
}

impl std::error::Error for PairedCompilerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Snapshot(error) => Some(error),
            Self::Inspect { source, .. }
            | Self::Spawn { source, .. }
            | Self::ReadStream { source, .. }
            | Self::Wait(source) => Some(source),
            Self::CapabilityFrame(error) => Some(error),
            Self::ExpectedCapability(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use scoop_protocol::encode_machine_capability_frame;

    use super::*;

    fn write_fake_compiler(path: &Path, frame: &[u8]) {
        let escaped = frame
            .iter()
            .map(|byte| format!("\\{:03o}", byte))
            .collect::<String>();
        std::fs::write(path, format!("#!/bin/sh\nprintf '{escaped}'\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn resolves_only_the_exact_shared_machine_identity() {
        let directory = tempfile::tempdir().unwrap();
        let compiler = directory.path().join("scoopc");
        let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
        write_fake_compiler(
            &compiler,
            &encode_machine_capability_frame(&expected).unwrap(),
        );
        let locator = PairedScoopcLocator::new(&compiler).unwrap();

        let resolved = ResolvedPairedScoopc::resolve(&locator).unwrap();

        assert_eq!(resolved.executable_path(), compiler.canonicalize().unwrap());
        assert_eq!(resolved.protocol(), expected.protocol());
        assert_eq!(
            resolved.fingerprint().toolchain_distribution_id(),
            expected.toolchain_distribution_id()
        );
    }

    #[test]
    fn rejects_non_executable_and_mismatched_tools() {
        let directory = tempfile::tempdir().unwrap();
        let compiler = directory.path().join("scoopc");
        std::fs::write(&compiler, b"not executable").unwrap();
        let locator = PairedScoopcLocator::new(&compiler).unwrap();
        assert!(matches!(
            ResolvedPairedScoopc::resolve(&locator),
            Err(PairedCompilerError::NotExecutable(_))
        ));

        let expected = scoop_toolchain::paired_compiler_machine_capability().unwrap();
        let mismatched = ScoopcMachineCapabilityV1::new(
            expected.protocol(),
            scoop_wire::sha256(b"another distribution"),
            expected.compiler_build_identity(),
            expected.identity_abi(),
        );
        write_fake_compiler(
            &compiler,
            &encode_machine_capability_frame(&mismatched).unwrap(),
        );
        assert!(matches!(
            ResolvedPairedScoopc::resolve(&locator),
            Err(PairedCompilerError::CapabilityMismatch { .. })
        ));
    }
}
