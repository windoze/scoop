use std::fmt;
use std::process::ExitStatus;

use scoop_protocol::{ProtocolReadError, ScoopcMachineCapabilityV1};
use scoop_wire::HashError;

#[derive(Debug)]
pub enum PairedCompilerError {
    RecordIo {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    RecordJson(serde_json::Error),
    Inspect {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    MissingExecutableParent(std::path::PathBuf),
    Spawn {
        path: std::path::PathBuf,
        source: std::io::Error,
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
            Self::RecordIo { path, source } => write!(
                formatter,
                "cannot access compiler record {}: {source}",
                path.display()
            ),
            Self::RecordJson(error) => {
                write!(formatter, "cannot read or write compiler record: {error}")
            }
            Self::Inspect { path, source } => {
                write!(
                    formatter,
                    "cannot inspect paired compiler {}: {source}",
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
            Self::RecordJson(error) => Some(error),
            Self::RecordIo { source, .. }
            | Self::Inspect { source, .. }
            | Self::Spawn { source, .. } => Some(source),
            Self::CapabilityFrame(error) => Some(error),
            Self::ExpectedCapability(error) => Some(error),
            _ => None,
        }
    }
}
