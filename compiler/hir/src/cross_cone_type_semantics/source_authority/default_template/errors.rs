use crate::*;
use scoop_identity::{CallableTemplateOrigin, LocalValueSelector};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceLocalIndexError {
    Resource(WireError),
    MissingSelector(LocalValueSelector),
}
impl fmt::Display for DefaultSourceLocalIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::MissingSelector(selector) => {
                write!(f, "source default local selector is absent: {selector:?}")
            }
        }
    }
}
impl std::error::Error for DefaultSourceLocalIndexError {}

#[derive(Debug)]
pub enum DefaultSourceTemplateIndexError {
    Resource(WireError),
    Body(ExportDefaultBodyIndexError<DefaultSourceLocalIndexError>),
    Receiver(TemplateReceiverIndexError<DefaultSourceLocalIndexError>),
    ValueParameters(TemplateValueParameterListIndexError<DefaultSourceLocalIndexError>),
}
impl fmt::Display for DefaultSourceTemplateIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Body(e) => e.fmt(f),
            Self::Receiver(e) => e.fmt(f),
            Self::ValueParameters(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for DefaultSourceTemplateIndexError {}

#[derive(Debug)]
pub enum DefaultSourceTemplateBuildError {
    Resource(WireError),
    Key(ProtectedSourceBuildError),
    ResultType,
    Index(DefaultSourceTemplateIndexError),
    ReferenceProvider {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
}
impl fmt::Display for DefaultSourceTemplateBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Key(e) => e.fmt(f),
            Self::ResultType => f.write_str("source default result differs from its body result"),
            Self::Index(e) => e.fmt(f),
            Self::ReferenceProvider {
                kind,
                index,
                expected,
                actual,
            } => write!(
                f,
                "source {kind} occurrence {index} provider {actual:?} differs from body provider {expected:?}"
            ),
        }
    }
}
impl std::error::Error for DefaultSourceTemplateBuildError {}

mod resolution;
pub use resolution::*;
