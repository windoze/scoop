//! Bounded process transport for the paired single-Cone compiler.

use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use scoop_protocol::{
    ProtocolReadError, ProtocolWriteError, ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1,
    ScoopcSuccessV1, decode_response_frame, encode_request_frame,
};
use scoop_slib::FingerprintAvailability;

use crate::{BuildArtifact, PairedCompilerError, ResolvedPairedScoopc};

const COMPILER_FAILURE_EXIT_CODE: i32 = 1;

#[derive(Debug, Default)]
pub(crate) struct ProductionSingleConeCompilerRunner;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChildIoPlan {
    trusted_sysroot: PathBuf,
}

impl ChildIoPlan {
    pub(crate) fn new(trusted_sysroot: PathBuf) -> Self {
        Self { trusted_sysroot }
    }

    pub(crate) fn trusted_sysroot(&self) -> &std::path::Path {
        &self.trusted_sysroot
    }
}

pub(crate) trait SingleConeCompilerRunner {
    fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError>;
}

impl SingleConeCompilerRunner for ProductionSingleConeCompilerRunner {
    fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        tool.revalidate_executable()
            .map_err(ChildTransportError::CompilerChanged)?;
        let request_frame = encode_request_frame(request).map_err(ChildTransportError::Request)?;
        let working_directory = tool.executable_path().parent().ok_or_else(|| {
            ChildTransportError::MissingExecutableParent {
                path: tool.executable_path().to_path_buf(),
            }
        })?;
        let mut child = Command::new(tool.executable_path())
            .arg("__child-protocol")
            .arg(tool.protocol().protocol_version().to_string())
            .env_clear()
            .env("SCOOP_SYSROOT", io.trusted_sysroot())
            .current_dir(working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ChildTransportError::Spawn {
                path: tool.executable_path().to_path_buf(),
                source,
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or(ChildTransportError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ChildTransportError::MissingPipe("stderr"))?;
        let stdout_thread = std::thread::spawn(move || read_stream(stdout, "stdout"));
        let stderr_thread = std::thread::spawn(move || read_stream(stderr, "stderr"));
        let write_result = child
            .stdin
            .take()
            .ok_or(ChildTransportError::MissingPipe("stdin"))
            .and_then(|mut stdin| {
                stdin
                    .write_all(&request_frame)
                    .and_then(|()| stdin.flush())
                    .map_err(ChildTransportError::WriteRequest)
            });
        let status = child.wait().map_err(ChildTransportError::Wait)?;
        let stdout = join_reader(stdout_thread, "stdout")?;
        let stderr = join_reader(stderr_thread, "stderr")?;
        write_result?;
        tool.revalidate_executable()
            .map_err(ChildTransportError::CompilerChanged)?;
        if status.code().is_none() {
            return Err(ChildTransportError::Signal);
        }
        let response =
            decode_response_frame(&stdout).map_err(|source| ChildTransportError::Response {
                status,
                stderr: stderr.clone(),
                source,
            })?;
        if response.request_id() != request.request_id() {
            return Err(ChildTransportError::RequestIdMismatch {
                expected: *request.request_id().as_array(),
                actual: *response.request_id().as_array(),
            });
        }
        if !stderr.is_empty() {
            return Err(ChildTransportError::UnexpectedStderr(stderr));
        }
        validate_exit(status, &response)?;
        Ok(response)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildSuccessArtifactField {
    ArtifactFingerprint,
    ConeIdentity,
    HirFingerprint,
    MirFingerprint,
    LirFingerprint,
    CodeFingerprint,
    RuntimeImageFingerprint,
    EmittedDumpDescriptors,
}

impl fmt::Display for ChildSuccessArtifactField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ArtifactFingerprint => "artifact fingerprint",
            Self::ConeIdentity => "Cone identity",
            Self::HirFingerprint => "HIR fingerprint",
            Self::MirFingerprint => "MIR fingerprint",
            Self::LirFingerprint => "LIR fingerprint",
            Self::CodeFingerprint => "Code fingerprint",
            Self::RuntimeImageFingerprint => "runtime-image fingerprint",
            Self::EmittedDumpDescriptors => "emitted dump descriptors",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildSuccessArtifactMismatch {
    field: ChildSuccessArtifactField,
}

impl ChildSuccessArtifactMismatch {
    pub const fn field(self) -> ChildSuccessArtifactField {
        self.field
    }
}

impl fmt::Display for ChildSuccessArtifactMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "child success response disagrees with validated {}",
            self.field
        )
    }
}

impl std::error::Error for ChildSuccessArtifactMismatch {}

pub(crate) fn validate_child_success_artifact(
    success: &ScoopcSuccessV1,
    artifact: &BuildArtifact,
) -> Result<(), ChildSuccessArtifactMismatch> {
    let summary = artifact.summary();
    let semantic = summary.semantic_fingerprints();
    validate_child_success_fields(
        success,
        ExpectedChildSuccessArtifact {
            artifact_fingerprint: *summary.artifact_fingerprint().as_array(),
            cone_identity: *summary.cone().identity().as_array(),
            hir_fingerprint: *semantic.hir().as_array(),
            mir_fingerprint: *semantic.mir().as_array(),
            lir_fingerprint: *semantic.lir().as_array(),
            code_fingerprint: available_fingerprint(semantic.code(), |value| *value.as_array()),
            runtime_image_fingerprint: available_fingerprint(semantic.runtime_image(), |value| {
                *value.as_array()
            }),
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedChildSuccessArtifact {
    artifact_fingerprint: [u8; 32],
    cone_identity: [u8; 32],
    hir_fingerprint: [u8; 32],
    mir_fingerprint: [u8; 32],
    lir_fingerprint: [u8; 32],
    code_fingerprint: Option<[u8; 32]>,
    runtime_image_fingerprint: Option<[u8; 32]>,
}

fn available_fingerprint<T>(
    availability: FingerprintAvailability<T>,
    to_array: impl FnOnce(T) -> [u8; 32],
) -> Option<[u8; 32]> {
    match availability {
        FingerprintAvailability::Unavailable => None,
        FingerprintAvailability::Available(value) => Some(to_array(value)),
    }
}

fn validate_child_success_fields(
    success: &ScoopcSuccessV1,
    expected: ExpectedChildSuccessArtifact,
) -> Result<(), ChildSuccessArtifactMismatch> {
    check_child_field(
        success.artifact_fingerprint().as_array() == &expected.artifact_fingerprint,
        ChildSuccessArtifactField::ArtifactFingerprint,
    )?;
    check_child_field(
        success.cone_identity().as_array() == &expected.cone_identity,
        ChildSuccessArtifactField::ConeIdentity,
    )?;
    check_child_field(
        success.hir_fingerprint().as_array() == &expected.hir_fingerprint,
        ChildSuccessArtifactField::HirFingerprint,
    )?;
    check_child_field(
        success.mir_fingerprint().as_array() == &expected.mir_fingerprint,
        ChildSuccessArtifactField::MirFingerprint,
    )?;
    check_child_field(
        success.lir_fingerprint().as_array() == &expected.lir_fingerprint,
        ChildSuccessArtifactField::LirFingerprint,
    )?;
    check_child_field(
        expected.code_fingerprint.as_ref() == Some(success.code_fingerprint().as_array()),
        ChildSuccessArtifactField::CodeFingerprint,
    )?;
    check_child_field(
        expected.runtime_image_fingerprint.as_ref()
            == Some(success.runtime_image_fingerprint().as_array()),
        ChildSuccessArtifactField::RuntimeImageFingerprint,
    )?;
    check_child_field(
        success.emitted_dump_descriptors().is_empty(),
        ChildSuccessArtifactField::EmittedDumpDescriptors,
    )
}

fn check_child_field(
    matches: bool,
    field: ChildSuccessArtifactField,
) -> Result<(), ChildSuccessArtifactMismatch> {
    if matches {
        Ok(())
    } else {
        Err(ChildSuccessArtifactMismatch { field })
    }
}

fn join_reader(
    thread: std::thread::JoinHandle<Result<Vec<u8>, ChildTransportError>>,
    name: &'static str,
) -> Result<Vec<u8>, ChildTransportError> {
    thread
        .join()
        .map_err(|_| ChildTransportError::ReaderPanicked(name))?
}

fn read_stream(mut stream: impl Read, name: &'static str) -> Result<Vec<u8>, ChildTransportError> {
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|source| ChildTransportError::ReadStream { name, source })?;
    Ok(bytes)
}

fn validate_exit(
    status: ExitStatus,
    response: &ScoopcResponseEnvelopeV1,
) -> Result<(), ChildTransportError> {
    let Some(code) = status.code() else {
        return Err(ChildTransportError::Signal);
    };
    let valid = matches!(response, ScoopcResponseEnvelopeV1::Success { .. }) && code == 0
        || matches!(response, ScoopcResponseEnvelopeV1::Failure { .. })
            && code == COMPILER_FAILURE_EXIT_CODE;
    if valid {
        Ok(())
    } else {
        Err(ChildTransportError::ExitMismatch {
            status,
            response: match response {
                ScoopcResponseEnvelopeV1::Success { .. } => ChildResponseKind::Success,
                ScoopcResponseEnvelopeV1::Failure { .. } => ChildResponseKind::Failure,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildResponseKind {
    Success,
    Failure,
}

#[derive(Debug)]
pub enum ChildTransportError {
    CompilerChanged(PairedCompilerError),
    Request(ProtocolWriteError),
    MissingExecutableParent {
        path: PathBuf,
    },
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },
    MissingPipe(&'static str),
    WriteRequest(std::io::Error),
    Wait(std::io::Error),
    ReadStream {
        name: &'static str,
        source: std::io::Error,
    },
    ReaderPanicked(&'static str),
    Response {
        status: ExitStatus,
        stderr: Vec<u8>,
        source: ProtocolReadError,
    },
    RequestIdMismatch {
        expected: [u8; 16],
        actual: [u8; 16],
    },
    UnexpectedStderr(Vec<u8>),
    Signal,
    ExitMismatch {
        status: ExitStatus,
        response: ChildResponseKind,
    },
}

impl fmt::Display for ChildTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerChanged(source) => {
                write!(
                    formatter,
                    "paired compiler changed around child invocation: {source}"
                )
            }
            Self::Request(source) => write!(formatter, "cannot encode child request: {source}"),
            Self::MissingExecutableParent { path } => write!(
                formatter,
                "paired compiler {} has no parent directory",
                path.display()
            ),
            Self::Spawn { path, source } => {
                write!(
                    formatter,
                    "cannot start paired compiler {}: {source}",
                    path.display()
                )
            }
            Self::MissingPipe(name) => write!(formatter, "child {name} pipe is unavailable"),
            Self::WriteRequest(source) => write!(formatter, "cannot write child request: {source}"),
            Self::Wait(source) => write!(formatter, "cannot wait for child: {source}"),
            Self::ReadStream { name, source } => {
                write!(formatter, "cannot read child {name}: {source}")
            }
            Self::ReaderPanicked(name) => write!(formatter, "child {name} reader panicked"),
            Self::Response {
                status,
                stderr,
                source,
            } => write!(
                formatter,
                "invalid child response after {status}: {source}; stderr={:?}",
                String::from_utf8_lossy(stderr)
            ),
            Self::RequestIdMismatch { expected, actual } => write!(
                formatter,
                "child response request id mismatch: expected {expected:02x?}, found {actual:02x?}"
            ),
            Self::UnexpectedStderr(stderr) => write!(
                formatter,
                "child wrote stderr alongside a protocol response: {:?}",
                String::from_utf8_lossy(stderr)
            ),
            Self::Signal => formatter.write_str("child terminated by signal"),
            Self::ExitMismatch { status, response } => write!(
                formatter,
                "child exit {status} contradicts its {response:?} response"
            ),
        }
    }
}

impl std::error::Error for ChildTransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompilerChanged(source) => Some(source),
            Self::Request(source) => Some(source),
            Self::Spawn { source, .. }
            | Self::WriteRequest(source)
            | Self::Wait(source)
            | Self::ReadStream { source, .. } => Some(source),
            Self::Response { source, .. } => Some(source),
            Self::MissingExecutableParent { .. }
            | Self::MissingPipe(_)
            | Self::ReaderPanicked(_)
            | Self::RequestIdMismatch { .. }
            | Self::UnexpectedStderr(_)
            | Self::Signal
            | Self::ExitMismatch { .. } => None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
