use std::fmt;

use scoop_identity::SourceOriginResolutionError;
use scoop_wire::{WireError, cbor::EncodeError};

use super::super::{
    ProtectedDefaultAccessWitnessResolutionError, ProtectedDefaultExpressionUsesResolutionError,
};
use super::ProtectedDefaultReferenceKindV1;
use crate::{ExportDefaultCallableTargetBuildError, ExportDefaultReferenceTargetResolutionError};

#[derive(Debug)]
pub enum ProtectedDefaultReferenceResolutionError<E> {
    Resource(WireError),
    Encoding(EncodeError),
    Target(ExportDefaultReferenceTargetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Witness(ProtectedDefaultAccessWitnessResolutionError<E>),
    Uses(ProtectedDefaultExpressionUsesResolutionError),
}
impl<E: fmt::Display> fmt::Display for ProtectedDefaultReferenceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::DefinitionOrigin(error) => {
                write!(f, "invalid reference definition origin: {error}")
            }
            Self::Witness(error) => write!(f, "invalid reference witness: {error}"),
            Self::Uses(error) => write!(f, "invalid reference expression uses: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultReferenceResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultReferenceSetBuildError {
    TooMany(ProtectedDefaultReferenceKindV1),
    Duplicate {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
    },
    NonCanonicalOrder {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
    },
    CallableTarget {
        index: usize,
        error: ExportDefaultCallableTargetBuildError,
    },
}
impl fmt::Display for ProtectedDefaultReferenceSetBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany(kind) => {
                write!(f, "protected default {kind} reference count exceeds u32")
            }
            Self::Duplicate { kind, index } => {
                write!(
                    f,
                    "duplicate protected default {kind} reference at index {index}"
                )
            }
            Self::NonCanonicalOrder { kind, index } => {
                write!(
                    f,
                    "non-canonical protected default {kind} reference order at index {index}"
                )
            }
            Self::CallableTarget { index, error } => {
                write!(
                    f,
                    "invalid protected default callable reference {index}: {error}"
                )
            }
        }
    }
}
impl std::error::Error for ProtectedDefaultReferenceSetBuildError {}

#[derive(Debug)]
pub enum ProtectedDefaultReferenceSetResolutionError<E> {
    Resource(WireError),
    Encoding(EncodeError),
    Record {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
        error: ProtectedDefaultReferenceResolutionError<E>,
    },
    Build(ProtectedDefaultReferenceSetBuildError),
}
impl<E: fmt::Display> fmt::Display for ProtectedDefaultReferenceSetResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Record { kind, index, error } => {
                write!(
                    f,
                    "invalid protected default {kind} reference {index}: {error}"
                )
            }
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultReferenceSetResolutionError<E>
{
}
