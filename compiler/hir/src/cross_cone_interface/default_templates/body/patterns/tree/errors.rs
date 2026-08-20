use std::fmt;

use crate::{
    DefaultEnumVariantRefResolutionError, DefaultExpressionIndexError,
    DefaultExpressionResolutionError, DefaultLiteralEqualityResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultPatternBuildError {
    InvalidLiteralExpression,
    TooManyElements,
    TooManyFields,
    DuplicateField {
        declaration_index: u32,
    },
    NonCanonicalFieldOrder {
        index: usize,
        previous: u32,
        actual: u32,
    },
}

impl fmt::Display for DefaultPatternBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLiteralExpression => formatter.write_str(
                "default literal pattern requires an integer, Boolean, or String literal",
            ),
            Self::TooManyElements => formatter.write_str("default tuple pattern exceeds u32"),
            Self::TooManyFields => formatter.write_str("default pattern field count exceeds u32"),
            Self::DuplicateField { declaration_index } => write!(
                formatter,
                "duplicate default pattern field {declaration_index}"
            ),
            Self::NonCanonicalFieldOrder {
                index,
                previous,
                actual,
            } => write!(
                formatter,
                "default pattern field {index} is out of order: {actual} follows {previous}"
            ),
        }
    }
}

impl std::error::Error for DefaultPatternBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultPatternResolutionError<E, L> {
    Local(L),
    LiteralValue(Box<DefaultExpressionResolutionError<E, L>>),
    LiteralEquality(DefaultLiteralEqualityResolutionError<E>),
    SubjectType(E),
    Variant(DefaultEnumVariantRefResolutionError<E>),
    Element { index: usize, error: Box<Self> },
    Field { index: usize, error: Box<Self> },
    StructOwnerType(E),
    Shape(DefaultPatternBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultPatternResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid default pattern local: {error}"),
            Self::LiteralValue(error) => {
                write!(formatter, "invalid default literal value: {error}")
            }
            Self::LiteralEquality(error) => {
                write!(formatter, "invalid default literal equality: {error}")
            }
            Self::SubjectType(error) => {
                write!(formatter, "invalid default literal subject type: {error}")
            }
            Self::Variant(error) => write!(formatter, "invalid default variant pattern: {error}"),
            Self::Element { index, error } => {
                write!(
                    formatter,
                    "invalid default tuple pattern element {index}: {error}"
                )
            }
            Self::Field { index, error } => {
                write!(formatter, "invalid default pattern field {index}: {error}")
            }
            Self::StructOwnerType(error) => {
                write!(formatter, "invalid default struct pattern owner: {error}")
            }
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultPatternResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultPatternIndexError<E> {
    Local(E),
    LiteralValue(Box<DefaultExpressionIndexError<E>>),
    Element { index: usize, error: Box<Self> },
    Field { index: usize, error: Box<Self> },
}

impl<E: fmt::Display> fmt::Display for DefaultPatternIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "cannot index default pattern local: {error}"),
            Self::LiteralValue(error) => {
                write!(formatter, "cannot index default literal value: {error}")
            }
            Self::Element { index, error } => {
                write!(
                    formatter,
                    "cannot index default tuple pattern element {index}: {error}"
                )
            }
            Self::Field { index, error } => {
                write!(
                    formatter,
                    "cannot index default pattern field {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultPatternIndexError<E> {}
