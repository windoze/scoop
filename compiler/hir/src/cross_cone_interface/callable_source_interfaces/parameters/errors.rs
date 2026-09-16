use std::fmt;

use scoop_identity::{CanonicalIdentifier, CanonicalIdentifierError, SourceOriginResolutionError};

#[derive(Debug, Eq, PartialEq)]
pub enum CallableParameterCallingResolutionError<E, T> {
    ElementType(E),
    Template(T),
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display
    for CallableParameterCallingResolutionError<E, T>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementType(error) => write!(formatter, "invalid vararg element type: {error}"),
            Self::Template(error) => write!(formatter, "invalid default template: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
    for CallableParameterCallingResolutionError<E, T>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceParameterResolutionError<E, T> {
    Name(CanonicalIdentifierError),
    ValueType(E),
    Calling(CallableParameterCallingResolutionError<E, T>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display
    for CallableSourceParameterResolutionError<E, T>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => write!(formatter, "invalid source parameter name: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid source parameter type: {error}"),
            Self::Calling(error) => write!(formatter, "invalid source parameter calling: {error}"),
            Self::DefinitionOrigin(error) => {
                write!(
                    formatter,
                    "invalid source parameter definition origin: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
    for CallableSourceParameterResolutionError<E, T>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSourceParameterListBuildError {
    TooMany,
    DuplicateName {
        position: u32,
        name: CanonicalIdentifier,
    },
    MultipleVarargs {
        first: u32,
        duplicate: u32,
    },
}

impl fmt::Display for CallableSourceParameterListBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("source parameter count exceeds u32"),
            Self::DuplicateName { position, name } => write!(
                formatter,
                "duplicate source parameter name {name:?} at position {position}"
            ),
            Self::MultipleVarargs { first, duplicate } => write!(
                formatter,
                "multiple vararg parameters at positions {first} and {duplicate}"
            ),
        }
    }
}

impl std::error::Error for CallableSourceParameterListBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceParameterListResolutionError<E, T> {
    Parameter {
        index: usize,
        error: CallableSourceParameterResolutionError<E, T>,
    },
    List(CallableSourceParameterListBuildError),
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display
    for CallableSourceParameterListResolutionError<E, T>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameter { index, error } => {
                write!(formatter, "invalid source parameter {index}: {error}")
            }
            Self::List(error) => write!(formatter, "invalid source parameter list: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
    for CallableSourceParameterListResolutionError<E, T>
{
}
