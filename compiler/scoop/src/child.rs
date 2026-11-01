//! Bounded process transport for the paired single-Cone compiler.

use scoop_process::CommandExt;
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use scoop_protocol::{
    ProtocolReadError, ProtocolWriteError, ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1,
    decode_response_frame, encode_request_frame,
};

use crate::ResolvedPairedScoopc;

const COMPILER_FAILURE_EXIT_CODE: i32 = 1;

#[derive(Debug, Default)]
pub(crate) struct ProductionSingleConeCompilerRunner;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChildIoPlan {
    temporary_root: PathBuf,
    pub(crate) jobs: std::num::NonZeroUsize,
    compiler_environment: Vec<(&'static str, std::ffi::OsString)>,
}

impl ChildIoPlan {
    pub(crate) fn new(temporary_root: PathBuf) -> Self {
        Self {
            temporary_root,
            jobs: std::num::NonZeroUsize::MIN,
            compiler_environment: Vec::new(),
        }
    }

    pub(crate) fn with_toolchain(
        mut self,
        invocation: &scoop_toolchain::ValidatedCBridgeToolchainInvocation,
    ) -> Self {
        self.compiler_environment = invocation
            .driver_environment()
            .map(|(name, value)| (name, value.to_owned()))
            .collect();
        self
    }
}

pub(crate) trait SingleConeCompilerRunner: Sync {
    fn invoke(
        &self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError>;
}

impl SingleConeCompilerRunner for ProductionSingleConeCompilerRunner {
    fn invoke(
        &self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        let request_frame = encode_request_frame(request).map_err(ChildTransportError::Request)?;
        let working_directory = tool.executable_path().parent().ok_or_else(|| {
            ChildTransportError::MissingExecutableParent {
                path: tool.executable_path().to_path_buf(),
            }
        })?;
        let temporary = tempfile::Builder::new()
            .prefix(".scoop-child-")
            .tempdir_in(&io.temporary_root)
            .map_err(ChildTransportError::TemporaryDirectory)?;
        let mut command = crate::compiler::compiler_command(tool.executable_path());
        #[cfg(windows)]
        command
            .env("TEMP", temporary.path())
            .env("TMP", temporary.path());
        let mut child = command
            .arg("__child-protocol")
            .arg(tool.protocol().protocol_version().to_string())
            .arg("--jobs")
            .arg(io.jobs.to_string())
            .envs(
                std::env::vars_os()
                    .filter(|(name, _)| name.as_encoded_bytes().starts_with(b"SCOOP_")),
            )
            .env("TMPDIR", temporary.path())
            .envs(
                io.compiler_environment
                    .iter()
                    .map(|(name, value)| (*name, value)),
            )
            .current_dir(working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .scoop_spawn()
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
        validate_exit(status, &response)?;
        if !stderr.is_empty() {
            std::io::stderr()
                .lock()
                .write_all(&stderr)
                .map_err(ChildTransportError::WriteLog)?;
        }
        Ok(response)
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
    let valid = matches!(
        response,
        ScoopcResponseEnvelopeV1::Success { .. } | ScoopcResponseEnvelopeV1::Observed { .. }
    ) && code == 0
        || matches!(response, ScoopcResponseEnvelopeV1::Failure { .. })
            && code == COMPILER_FAILURE_EXIT_CODE;
    if valid {
        Ok(())
    } else {
        Err(ChildTransportError::ExitMismatch {
            status,
            response: match response {
                ScoopcResponseEnvelopeV1::Success { .. }
                | ScoopcResponseEnvelopeV1::Observed { .. } => ChildResponseKind::Success,
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
    WorkerSpawn(std::io::Error),
    WorkerPanicked,
    Request(ProtocolWriteError),
    TemporaryDirectory(std::io::Error),
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
    WriteLog(std::io::Error),
    Signal,
    ExitMismatch {
        status: ExitStatus,
        response: ChildResponseKind,
    },
}

impl fmt::Display for ChildTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerSpawn(source) => {
                write!(formatter, "cannot start compiler worker: {source}")
            }
            Self::WorkerPanicked => formatter.write_str("compiler worker panicked"),
            Self::Request(source) => write!(formatter, "cannot encode child request: {source}"),
            Self::TemporaryDirectory(source) => write!(
                formatter,
                "cannot create compiler scratch directory: {source}"
            ),
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
            Self::WriteLog(source) => write!(formatter, "cannot write compiler log: {source}"),
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
            Self::Request(source) => Some(source),
            Self::Spawn { source, .. }
            | Self::WorkerSpawn(source)
            | Self::TemporaryDirectory(source)
            | Self::WriteRequest(source)
            | Self::WriteLog(source)
            | Self::Wait(source)
            | Self::ReadStream { source, .. } => Some(source),
            Self::Response { source, .. } => Some(source),
            Self::MissingExecutableParent { .. }
            | Self::MissingPipe(_)
            | Self::ReaderPanicked(_)
            | Self::WorkerPanicked
            | Self::RequestIdMismatch { .. }
            | Self::Signal
            | Self::ExitMismatch { .. } => None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
