use std::fmt;

use scoop_manifest::{SingleFileInputError, SourceDiscoveryError};

use super::super::staging::StagingError;
use crate::PairedCompilerError;

#[derive(Debug)]
pub enum PrepareBuildGraphError {
    PairedCompiler(PairedCompilerError),
    Staging(StagingError),
    SourceDiscovery(SourceDiscoveryError),
    Native(scoop_toolchain::ToolchainError),
    SingleFile(SingleFileInputError),
}

impl fmt::Display for PrepareBuildGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(error) => error.fmt(formatter),
            Self::PairedCompiler(error) => error.fmt(formatter),
            Self::Staging(error) => error.fmt(formatter),
            Self::SourceDiscovery(error) => {
                write!(formatter, "cannot snapshot manifest sources: {error}")
            }
            Self::SingleFile(error) => {
                write!(formatter, "cannot snapshot single-file root: {error}")
            }
        }
    }
}

impl std::error::Error for PrepareBuildGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::PairedCompiler(error) => Some(error),
            Self::Staging(error) => Some(error),
            Self::SourceDiscovery(error) => Some(error),
            Self::SingleFile(error) => Some(error),
        }
    }
}
