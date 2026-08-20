use std::fmt;

use scoop_identity::{CanonicalIdentifier, CanonicalIdentifierError, SignatureTypeKey};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureTypeSetBuildError {
    Duplicate(Box<SignatureTypeKey>),
}

impl fmt::Display for SignatureTypeSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(value) => write!(formatter, "duplicate signature type {value:?}"),
        }
    }
}

impl std::error::Error for SignatureTypeSetBuildError {}

#[derive(Debug)]
pub enum SignatureTypeSetValidationError<E> {
    Reference { index: usize, error: E },
    Duplicate { index: usize },
    NonCanonicalOrder { index: usize },
}

impl<E: fmt::Display> fmt::Display for SignatureTypeSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference { index, error } => {
                write!(formatter, "invalid signature type {index}: {error}")
            }
            Self::Duplicate { index } => write!(formatter, "duplicate signature type at {index}"),
            Self::NonCanonicalOrder { index } => {
                write!(formatter, "non-canonical signature type order at {index}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SignatureTypeSetValidationError<E> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeParameterBoundsBuildError {
    EmptyNominal,
}

impl fmt::Display for TypeParameterBoundsBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("nominal type-parameter bounds must not be empty")
    }
}

impl std::error::Error for TypeParameterBoundsBuildError {}

#[derive(Debug)]
pub enum TypeParameterBoundsResolutionError<E> {
    Reference(E),
    Interfaces(SignatureTypeSetValidationError<E>),
    Shape(TypeParameterBoundsBuildError),
}

impl<E: fmt::Display> fmt::Display for TypeParameterBoundsResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => write!(formatter, "invalid class bound: {error}"),
            Self::Interfaces(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TypeParameterBoundsResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeParameterBinderBuildError {
    TooMany,
    DuplicateName(CanonicalIdentifier),
}

impl fmt::Display for TypeParameterBinderBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("type-parameter count exceeds u32"),
            Self::DuplicateName(name) => write!(formatter, "duplicate type-parameter name {name}"),
        }
    }
}

impl std::error::Error for TypeParameterBinderBuildError {}

#[derive(Debug)]
pub enum TypeParameterBinderResolutionError<E> {
    Name(CanonicalIdentifierError),
    Bounds(TypeParameterBoundsResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for TypeParameterBinderResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => write!(formatter, "invalid type-parameter name: {error}"),
            Self::Bounds(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TypeParameterBinderResolutionError<E> {}

#[derive(Debug)]
pub enum BinderListValidationError<E> {
    TooMany,
    Binder {
        index: usize,
        error: TypeParameterBinderResolutionError<E>,
    },
    DuplicateName {
        index: usize,
        name: CanonicalIdentifier,
    },
}

impl<E: fmt::Display> fmt::Display for BinderListValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("type-parameter count exceeds u32"),
            Self::Binder { index, error } => {
                write!(formatter, "invalid type parameter {index}: {error}")
            }
            Self::DuplicateName { index, name } => {
                write!(
                    formatter,
                    "duplicate type-parameter name {name} at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BinderListValidationError<E> {}
