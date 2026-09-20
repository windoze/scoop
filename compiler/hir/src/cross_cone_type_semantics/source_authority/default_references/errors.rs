use crate::*;
use scoop_identity::SourceOriginResolutionError;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultSourceReferencesBuildError {
    TooMany(ExportDefaultReferenceKindV1),
    CallableTarget {
        index: usize,
        error: ExportDefaultCallableTargetBuildError,
    },
}
impl fmt::Display for DefaultSourceReferencesBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany(kind) => write!(f, "source default {kind} occurrence count exceeds u32"),
            Self::CallableTarget { index, error } => {
                write!(f, "invalid source callable occurrence {index}: {error}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceReferencesBuildError {}

#[derive(Debug)]
pub enum DefaultSourceReferenceResolutionError<E> {
    Resource(WireError),
    Target(ExportDefaultReferenceTargetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Witness(DefaultSourceAccessResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for DefaultSourceReferenceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Target(e) => e.fmt(f),
            Self::DefinitionOrigin(e) => write!(f, "invalid source occurrence origin: {e}"),
            Self::Witness(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for DefaultSourceReferenceResolutionError<E>
{
}
#[derive(Debug)]
pub enum DefaultSourceReferencesResolutionError<E> {
    Resource(WireError),
    Build(DefaultSourceReferencesBuildError),
    Record {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        error: DefaultSourceReferenceResolutionError<E>,
    },
}
impl<E: fmt::Display> fmt::Display for DefaultSourceReferencesResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::Record { kind, index, error } => {
                write!(f, "invalid source {kind} occurrence {index}: {error}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for DefaultSourceReferencesResolutionError<E>
{
}
impl<E> DefaultSourceReferencesResolutionError<E> {
    pub fn resource_error(&self) -> Option<&WireError> {
        match self {
            Self::Resource(e)
            | Self::Record {
                error: DefaultSourceReferenceResolutionError::Resource(e),
                ..
            } => Some(e),
            Self::Record {
                error: DefaultSourceReferenceResolutionError::Witness(e),
                ..
            } => e.resource_error(),
            _ => None,
        }
    }
}
