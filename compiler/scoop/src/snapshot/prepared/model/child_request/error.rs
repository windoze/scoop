use scoop_identity::ConeIdentity;
use scoop_protocol::{HostPathError, ProtocolValidationError};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ChildRequestPlanError {
    NativeNotPrepared,
    UnknownNode(ConeIdentity),
    PrebuiltNode(ConeIdentity),
    MissingSourceProjection(ConeIdentity),
    CurrentAlreadyCompleted(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    MissingTrustedCore,
    MissingDependency {
        current: ConeIdentity,
        dependency: ConeIdentity,
        role: &'static str,
    },
    HostPath {
        path: PathBuf,
        source: HostPathError,
    },
    Protocol(ProtocolValidationError),
    Toolchain(scoop_toolchain::ToolchainError),
}

impl fmt::Display for ChildRequestPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeNotPrepared => {
                formatter.write_str("frontend-only source has no native production inputs")
            }
            Self::UnknownNode(identity) => write!(formatter, "unknown prepared Cone {identity}"),
            Self::PrebuiltNode(identity) => {
                write!(
                    formatter,
                    "prebuilt Cone {identity} must not launch a compiler child"
                )
            }
            Self::MissingSourceProjection(identity) => {
                write!(
                    formatter,
                    "prepared source Cone {identity} has no dependency projection"
                )
            }
            Self::CurrentAlreadyCompleted(identity) => {
                write!(formatter, "child Cone {identity} is already completed")
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::MissingTrustedCore => {
                formatter.write_str("ordinary compiler child requires completed trusted core")
            }
            Self::MissingDependency {
                current,
                dependency,
                role,
            } => write!(
                formatter,
                "child Cone {current} is missing completed {role} dependency {dependency}"
            ),
            Self::HostPath { path, source } => {
                write!(
                    formatter,
                    "cannot transport child path {}: {source}",
                    path.display()
                )
            }
            Self::Protocol(source) => write!(formatter, "invalid child request plan: {source}"),
            Self::Toolchain(source) => {
                write!(formatter, "cannot transport compiler selection: {source}")
            }
        }
    }
}

impl std::error::Error for ChildRequestPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HostPath { source, .. } => Some(source),
            Self::Protocol(source) => Some(source),
            Self::Toolchain(source) => Some(source),
            _ => None,
        }
    }
}
