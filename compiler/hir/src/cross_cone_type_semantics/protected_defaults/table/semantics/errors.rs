use crate::{
    DefaultBodyProviderEnvelopeSemanticValidationError, DefaultNestedCallableAbiValidationError,
    ExportDefaultBodyOperationTypingValidationError, ExportDefaultLocalDataFlowValidationError,
    ProtectedDefaultReferenceSemanticError, ProtectedDefaultTemplateContractSemanticError,
    ProtectedDefaultTemplateKeyV1, ProtectedDefaultTemplateOriginSemanticError,
    ProtectedSourceClosureError, ProtectedSourceIndexError,
};
use scoop_wire::WireError;

#[derive(Debug)]
pub enum ProtectedDefaultTableSemanticError<E> {
    Resource(WireError),
    SourceClosure(ProtectedSourceIndexError),
    MissingSource {
        index: usize,
        key: ProtectedDefaultTemplateKeyV1,
    },
    Source {
        index: usize,
        key: ProtectedDefaultTemplateKeyV1,
        error: Box<ProtectedSourceClosureError<E>>,
    },
    Template {
        index: usize,
        key: ProtectedDefaultTemplateKeyV1,
        error: Box<ProtectedDefaultSemanticError<E>>,
    },
}
#[derive(Debug)]
pub enum ProtectedDefaultSemanticError<E> {
    Contract(Box<ProtectedDefaultTemplateContractSemanticError<E>>),
    BodyEnvelope(Box<DefaultBodyProviderEnvelopeSemanticValidationError<E>>),
    Origin(Box<ProtectedDefaultTemplateOriginSemanticError<E>>),
    LocalDataFlow(Box<ExportDefaultLocalDataFlowValidationError<E>>),
    OperationTyping(Box<ExportDefaultBodyOperationTypingValidationError<E>>),
    NestedCallableAbi(Box<DefaultNestedCallableAbiValidationError<E>>),
    References(Box<ProtectedDefaultReferenceSemanticError<E>>),
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultTableSemanticError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::SourceClosure(error) => error.fmt(f),
            Self::MissingSource { index, key } => write!(
                f,
                "protected default {index} ({key:?}) has no checked source protocol"
            ),
            Self::Source { index, key, error } => write!(
                f,
                "protected default {index} ({key:?}) has an invalid source: {error}"
            ),
            Self::Template { index, key, error } => {
                write!(f, "protected default {index} ({key:?}) is invalid: {error}")
            }
        }
    }
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultSemanticError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(f),
            Self::BodyEnvelope(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::LocalDataFlow(error) => error.fmt(f),
            Self::OperationTyping(error) => error.fmt(f),
            Self::NestedCallableAbi(error) => error.fmt(f),
            Self::References(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultTableSemanticError<E> {}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultSemanticError<E> {}
