use crate::{
    DefaultTemplateProviderShapeBuildError, DefaultTemplateRootSemanticValidationError,
    MeteredDefaultTemplateTypeSubstitutionError, MeteredSignatureTypeSemanticError,
    MeteredTemplateReceiverSemanticValidationError,
    MeteredTemplateValueParameterSemanticValidationError, TemplateLocalScopeValidationError,
};
use scoop_wire::WireError;

#[derive(Debug)]
pub enum ProtectedDefaultTemplateContractSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    SourceOwner,
    ProtocolOwner,
    ParameterOutOfRange {
        position: u32,
        arity: usize,
    },
    ParameterTemplate,
    OwnerShape,
    ProviderShape(DefaultTemplateProviderShapeBuildError),
    DefinitionRoot(DefaultTemplateRootSemanticValidationError<E>),
    ProviderOwnerShape,
    ProviderOwnerReceiver,
    DirectMapping {
        index: usize,
    },
    MappingArity {
        expected: u32,
        actual: u32,
    },
    MappingArgument {
        index: usize,
        error: MeteredSignatureTypeSemanticError<E>,
    },
    LocalScope(TemplateLocalScopeValidationError),
    LocalType {
        index: usize,
        error: MeteredSignatureTypeSemanticError<E>,
    },
    Receiver(MeteredTemplateReceiverSemanticValidationError),
    ValueParameters(MeteredTemplateValueParameterSemanticValidationError),
    ResultType(MeteredSignatureTypeSemanticError<E>),
    ResultSubstitution(MeteredDefaultTemplateTypeSubstitutionError),
    ResultMismatch,
    SuspendPermission,
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultTemplateContractSemanticError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::ProviderShape(error) => error.fmt(f),
            Self::DefinitionRoot(error) => error.fmt(f),
            Self::LocalScope(error) => error.fmt(f),
            Self::Receiver(error) => error.fmt(f),
            Self::ValueParameters(error) => error.fmt(f),
            Self::ResultType(error) => error.fmt(f),
            Self::ResultSubstitution(error) => error.fmt(f),
            Self::SourceOwner => {
                f.write_str("protected default owner differs from its checked callable source")
            }
            Self::ProtocolOwner => {
                f.write_str("protected default owner differs from its checked omission protocol")
            }
            Self::ParameterOutOfRange { position, arity } => write!(
                f,
                "protected default parameter {position} is outside source arity {arity}"
            ),
            Self::ParameterTemplate => {
                f.write_str("source parameter does not reference this protected default key")
            }
            Self::OwnerShape => {
                f.write_str("protected default source owner has an invalid nominal binder shape")
            }
            Self::ProviderOwnerShape => {
                f.write_str("direct default provider binder shape differs from its checked source")
            }
            Self::ProviderOwnerReceiver => {
                f.write_str("direct default provider receiver differs from its checked source")
            }
            Self::DirectMapping { index } => write!(
                f,
                "direct default provider binder mapping is not identity at argument {index}"
            ),
            Self::MappingArity { expected, actual } => write!(
                f,
                "protected default provider has {expected} binders, mapping has {actual}"
            ),
            Self::MappingArgument { index, error } => write!(
                f,
                "protected default mapping argument {index} is invalid: {error}"
            ),
            Self::LocalType { index, error } => write!(
                f,
                "protected default local {index} type is invalid: {error}"
            ),
            Self::ResultMismatch => f.write_str(
                "mapped protected default result differs from its source parameter type",
            ),
            Self::SuspendPermission => {
                f.write_str("protected default suspend permission differs from its source callable")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultTemplateContractSemanticError<E>
{
}
