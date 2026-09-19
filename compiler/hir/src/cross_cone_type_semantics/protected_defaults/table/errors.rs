use std::fmt;

use crate::{
    ProtectedDefaultTemplateKeyV1, ProtectedDefaultTemplateResolutionError,
    ProtectedSourceBuildError,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultTemplateTableBuildError {
    TooMany,
    Duplicate(ProtectedDefaultTemplateKeyV1),
    NonCanonicalOrder { index: usize },
    KeyIndex(ProtectedSourceBuildError),
}
impl fmt::Display for ProtectedDefaultTemplateTableBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => f.write_str("protected default template count exceeds u32"),
            Self::Duplicate(key) => write!(f, "duplicate protected default template {key:?}"),
            Self::NonCanonicalOrder { index } => write!(
                f,
                "noncanonical protected default template order at {index}"
            ),
            Self::KeyIndex(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ProtectedDefaultTemplateTableBuildError {}

#[derive(Debug)]
pub enum ProtectedDefaultTemplateTableResolutionError<E> {
    Resource(scoop_wire::WireError),
    Build(ProtectedDefaultTemplateTableBuildError),
    Template {
        index: usize,
        error: ProtectedDefaultTemplateResolutionError<E>,
    },
}
impl<E: fmt::Display> fmt::Display for ProtectedDefaultTemplateTableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
            Self::Template { index, error } => {
                write!(f, "invalid protected default template {index}: {error}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultTemplateTableResolutionError<E>
{
}
