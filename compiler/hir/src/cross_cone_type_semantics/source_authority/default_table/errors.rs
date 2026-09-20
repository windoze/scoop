use crate::*;
use scoop_wire::WireError;
use std::fmt;
#[derive(Debug)]
pub enum DefaultSourceTemplateTableBuildError {
    Resource(WireError),
    TooMany,
    Duplicate(ProtectedDefaultTemplateKeyV1),
    NonCanonicalOrder { index: usize },
}
impl fmt::Display for DefaultSourceTemplateTableBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::TooMany => f.write_str("source default template count exceeds u32"),
            Self::Duplicate(key) => write!(f, "duplicate source default key {key:?}"),
            Self::NonCanonicalOrder { index } => {
                write!(f, "non-canonical source default order at {index}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceTemplateTableBuildError {}
#[derive(Debug)]
pub enum DefaultSourceTemplateTableResolutionError<E> {
    Resource(WireError),
    Template {
        index: usize,
        error: DefaultSourceTemplateResolutionError<E>,
    },
    Table(DefaultSourceTemplateTableBuildError),
}
impl<E: fmt::Display> fmt::Display for DefaultSourceTemplateTableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Template { index, error } => {
                write!(f, "source default template {index}: {error}")
            }
            Self::Table(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for DefaultSourceTemplateTableResolutionError<E>
{
}
#[derive(Debug)]
pub enum DefaultSourceTemplateTableIndexError {
    Resource(WireError),
    Template {
        index: usize,
        error: DefaultSourceTemplateIndexError,
    },
}
impl fmt::Display for DefaultSourceTemplateTableIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Template { index, error } => {
                write!(f, "source default template {index}: {error}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceTemplateTableIndexError {}
