//! Bounded process transport for the paired single-Cone compiler.

use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use scoop_protocol::{
    ProtocolReadError, ProtocolWriteError, ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1,
    decode_response_frame, encode_request_frame,
};

use crate::{PairedCompilerError, ResolvedPairedScoopc};

const FRAME_PREFIX_BYTES: usize = 8;
const MAX_CHILD_STDERR_BYTES: u64 = 65_536;
const COMPILER_FAILURE_EXIT_CODE: i32 = 1;

#[derive(Debug, Default)]
pub struct ProductionSingleConeCompilerRunner;

impl ProductionSingleConeCompilerRunner {
    pub fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
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
        let stdout_thread = std::thread::spawn(move || {
            read_bounded_stream(
                stdout,
                "stdout",
                child_stdout_limit().ok_or(ChildTransportError::StreamLengthOverflow("stdout"))?,
            )
        });
        let stderr_thread = std::thread::spawn(move || {
            read_bounded_stream(stderr, "stderr", MAX_CHILD_STDERR_BYTES)
        });
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

fn child_stdout_limit() -> Option<u64> {
    scoop_protocol::PROTOCOL_MAX_FRAME_BYTES
        .checked_add(FRAME_PREFIX_BYTES)
        .and_then(|limit| u64::try_from(limit).ok())
}

fn join_reader(
    thread: std::thread::JoinHandle<Result<Vec<u8>, ChildTransportError>>,
    name: &'static str,
) -> Result<Vec<u8>, ChildTransportError> {
    thread
        .join()
        .map_err(|_| ChildTransportError::ReaderPanicked(name))?
}

fn read_bounded_stream(
    stream: impl Read,
    name: &'static str,
    limit: u64,
) -> Result<Vec<u8>, ChildTransportError> {
    let read_limit = limit
        .checked_add(1)
        .ok_or(ChildTransportError::StreamLengthOverflow(name))?;
    let mut bytes = Vec::new();
    stream
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| ChildTransportError::ReadStream { name, source })?;
    let observed =
        u64::try_from(bytes.len()).map_err(|_| ChildTransportError::StreamLengthOverflow(name))?;
    if observed > limit {
        return Err(ChildTransportError::StreamTooLarge {
            name,
            limit,
            observed,
        });
    }
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
    StreamLengthOverflow(&'static str),
    StreamTooLarge {
        name: &'static str,
        limit: u64,
        observed: u64,
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
            Self::StreamLengthOverflow(name) => {
                write!(formatter, "child {name} byte limit overflowed")
            }
            Self::StreamTooLarge {
                name,
                limit,
                observed,
            } => write!(
                formatter,
                "child {name} exceeds byte limit {limit}: observed {observed}"
            ),
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
            | Self::StreamLengthOverflow(_)
            | Self::StreamTooLarge { .. }
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
