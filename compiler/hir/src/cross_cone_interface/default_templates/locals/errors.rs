use std::fmt;

use scoop_identity::{LocalValueSelector, SourceOriginResolutionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateLocalRecordBuildError {
    UnsupportedSelector(LocalValueSelector),
    SourceDefinitionRequired(LocalValueSelector),
    SyntheticDefinitionRequired(LocalValueSelector),
}

impl fmt::Display for TemplateLocalRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSelector(selector) => {
                write!(
                    formatter,
                    "unsupported default-template local selector {selector:?}"
                )
            }
            Self::SourceDefinitionRequired(selector) => write!(
                formatter,
                "default-template local selector {selector:?} requires a source definition"
            ),
            Self::SyntheticDefinitionRequired(selector) => write!(
                formatter,
                "default-template local selector {selector:?} requires a synthetic definition"
            ),
        }
    }
}

impl std::error::Error for TemplateLocalRecordBuildError {}

#[derive(Debug)]
pub enum TemplateLocalRecordResolutionError<E> {
    Type(E),
    Definition(SourceOriginResolutionError<E>),
    Shape(TemplateLocalRecordBuildError),
}

impl<E: fmt::Display> fmt::Display for TemplateLocalRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type(error) => write!(formatter, "invalid default-template local type: {error}"),
            Self::Definition(error) => {
                write!(
                    formatter,
                    "invalid default-template local definition: {error}"
                )
            }
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TemplateLocalRecordResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateLocalTableBuildError {
    TooMany,
    Duplicate(LocalValueSelector),
}

impl fmt::Display for TemplateLocalTableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("default-template local count exceeds u32"),
            Self::Duplicate(selector) => {
                write!(formatter, "duplicate default-template local {selector:?}")
            }
        }
    }
}

impl std::error::Error for TemplateLocalTableBuildError {}

#[derive(Debug)]
pub enum TemplateLocalTableValidationError<E> {
    TooMany,
    Record {
        index: usize,
        error: TemplateLocalRecordResolutionError<E>,
    },
    Duplicate {
        index: usize,
        selector: LocalValueSelector,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for TemplateLocalTableValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("default-template local count exceeds u32"),
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid default-template local at index {index}: {error}"
                )
            }
            Self::Duplicate { index, selector } => write!(
                formatter,
                "duplicate default-template local {selector:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical default-template local order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TemplateLocalTableValidationError<E> {}
