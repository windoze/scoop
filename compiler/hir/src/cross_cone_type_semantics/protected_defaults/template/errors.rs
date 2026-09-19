use std::fmt;

use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey, SourceOriginResolutionError};
use scoop_wire::{WireError, cbor::EncodeError};

use super::super::{ProtectedDefaultReferenceKindV1, ProtectedDefaultReferenceSetResolutionError};
use super::ProtectedDefaultTemplateIndexError;
use crate::{
    BinderUseListValidationError, ExportDefaultBodyResolutionError, ProtectedSourceResolutionError,
    TemplateLocalLookupError, TemplateLocalTableValidationError, TemplateReceiverResolutionError,
    TemplateValueParameterListValidationError,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ProtectedDefaultTemplateBuildError {
    ResultType {
        declared: SignatureTypeKey,
        body: SignatureTypeKey,
    },
    LocalIndices(ProtectedDefaultTemplateIndexError),
    ReferenceOwner {
        kind: ProtectedDefaultReferenceKindV1,
        index: usize,
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
}
impl fmt::Display for ProtectedDefaultTemplateBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResultType { declared, body } => write!(
                f,
                "protected default result {declared:?} differs from body result {body:?}"
            ),
            Self::LocalIndices(error) => error.fmt(f),
            Self::ReferenceOwner {
                kind,
                index,
                expected,
                actual,
            } => write!(
                f,
                "protected default {kind} reference {index} has witness owner {actual:?}, expected {expected:?}"
            ),
        }
    }
}
impl std::error::Error for ProtectedDefaultTemplateBuildError {}

#[derive(Debug)]
pub enum ProtectedDefaultTemplateResolutionError<E> {
    Resource(WireError),
    Encoding(EncodeError),
    Key(ProtectedSourceResolutionError<E>),
    DefinitionRoot(E),
    Locals(TemplateLocalTableValidationError<E>),
    Body(ExportDefaultBodyResolutionError<E, TemplateLocalLookupError>),
    Result(E),
    TypeParameters(BinderUseListValidationError<E>),
    Receiver(TemplateReceiverResolutionError<E, TemplateLocalLookupError>),
    ValueParameters(TemplateValueParameterListValidationError<TemplateLocalLookupError>),
    References(ProtectedDefaultReferenceSetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(ProtectedDefaultTemplateBuildError),
}
impl<E: fmt::Display> fmt::Display for ProtectedDefaultTemplateResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Key(error) => write!(f, "invalid protected default key: {error}"),
            Self::DefinitionRoot(error) => write!(f, "invalid protected default root: {error}"),
            Self::Locals(error) => write!(f, "invalid protected default locals: {error}"),
            Self::Body(error) => write!(f, "invalid protected default body: {error}"),
            Self::Result(error) => write!(f, "invalid protected default result: {error}"),
            Self::TypeParameters(error) => {
                write!(f, "invalid protected default binder uses: {error}")
            }
            Self::Receiver(error) => write!(f, "invalid protected default receiver: {error}"),
            Self::ValueParameters(error) => {
                write!(f, "invalid protected default parameters: {error}")
            }
            Self::References(error) => write!(f, "invalid protected default references: {error}"),
            Self::DefinitionOrigin(error) => write!(f, "invalid protected default origin: {error}"),
            Self::Record(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultTemplateResolutionError<E>
{
}
