//! Errors from the shared semantic and object reader and identity import.
use std::fmt;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeClosureArtifactSlotV1 {
    Dependency(usize),
    Current,
}

impl fmt::Display for CrossConeClosureArtifactSlotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dependency(index) => write!(formatter, "dependency artifact {index}"),
            Self::Current => formatter.write_str("current artifact"),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeArtifactClosureValidationError {
    Layout(Box<crate::CrossConeLayoutArtifactValidationError>),
    Commit(Box<super::super::CrossConeSemanticCommitError>),
    MissingCompletedCurrentArtifact,
}
impl fmt::Display for CrossConeArtifactClosureValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::Commit(error) => error.fmt(f),
            Self::MissingCompletedCurrentArtifact => {
                f.write_str("completed artifact is missing from its dependency closure")
            }
        }
    }
}
impl std::error::Error for CrossConeArtifactClosureValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error.as_ref()),
            Self::Commit(error) => Some(error.as_ref()),
            Self::MissingCompletedCurrentArtifact => None,
        }
    }
}
