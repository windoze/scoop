use std::fmt;

use scoop_identity::{CanonicalIdentifier, CanonicalIdentifierError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceParameterListBuildError {
    TooMany,
    DuplicateName(CanonicalIdentifier),
}

impl fmt::Display for SourceParameterListBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("source parameter count exceeds u32"),
            Self::DuplicateName(name) => {
                write!(formatter, "duplicate source parameter name {name}")
            }
        }
    }
}

impl std::error::Error for SourceParameterListBuildError {}

#[derive(Debug)]
pub enum SourceParameterShapeResolutionError<E> {
    Name(CanonicalIdentifierError),
    ValueType(E),
}

impl<E: fmt::Display> fmt::Display for SourceParameterShapeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => write!(formatter, "invalid source parameter name: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid source parameter type: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceParameterShapeResolutionError<E> {}

#[derive(Debug)]
pub enum SourceParameterListValidationError<E> {
    TooMany,
    Parameter {
        index: usize,
        error: SourceParameterShapeResolutionError<E>,
    },
    DuplicateName {
        index: usize,
        name: CanonicalIdentifier,
    },
}

impl<E: fmt::Display> fmt::Display for SourceParameterListValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("source parameter count exceeds u32"),
            Self::Parameter { index, error } => {
                write!(formatter, "invalid source parameter {index}: {error}")
            }
            Self::DuplicateName { index, name } => {
                write!(
                    formatter,
                    "duplicate source parameter name {name} at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceParameterListValidationError<E> {}
