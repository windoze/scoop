use crate::{
    DefaultTemplateTypeSubstitutionError, SignatureTypeSemanticError,
    TemplateReceiverSemanticValidationError, TemplateValueParameterSemanticValidationError,
};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultTemplateDeclarationContractError<E> {
    Resource(WireError),
    Declaration,
    ParameterPosition,
    ParameterArity,
    DefinitionPath,
    MappingArity,
    DirectShape,
    DirectMapping { index: usize },
    ParameterType { index: usize },
    ResultType,
    SuspendPermission,
    Signature(Box<SignatureTypeSemanticError<E>>),
    Receiver(TemplateReceiverSemanticValidationError),
    Prefix(TemplateValueParameterSemanticValidationError),
    Substitution(DefaultTemplateTypeSubstitutionError),
}

impl<E> From<WireError> for DefaultTemplateDeclarationContractError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl<E: fmt::Display> fmt::Display for DefaultTemplateDeclarationContractError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Declaration => {
                f.write_str("default contract belongs to another source declaration")
            }
            Self::ParameterPosition => {
                f.write_str("default provider and publisher parameter positions differ")
            }
            Self::ParameterArity => {
                f.write_str("default source provider and owner parameter counts differ")
            }
            Self::DefinitionPath => {
                f.write_str("default source path does not identify its provider's default ordinal")
            }
            Self::MappingArity => {
                f.write_str("default source mapping differs from its provider binder count")
            }
            Self::DirectShape => f.write_str("direct default source binder shapes differ"),
            Self::DirectMapping { index } => write!(
                f,
                "direct default source mapping argument {index} is not identity"
            ),
            Self::ParameterType { index } => write!(
                f,
                "default source parameter {index} differs after provider substitution"
            ),
            Self::ResultType => {
                f.write_str("default source result differs from its original provider parameter")
            }
            Self::SuspendPermission => {
                f.write_str("default source suspend permission differs from its declarations")
            }
            Self::Signature(error) => error.fmt(f),
            Self::Receiver(error) => error.fmt(f),
            Self::Prefix(error) => error.fmt(f),
            Self::Substitution(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultTemplateDeclarationContractError<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Signature(error) => Some(error),
            Self::Receiver(error) => Some(error),
            Self::Prefix(error) => Some(error),
            Self::Substitution(error) => Some(error),
            _ => None,
        }
    }
}
