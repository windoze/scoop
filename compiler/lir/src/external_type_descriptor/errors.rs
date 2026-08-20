use scoop_identity::{
    IdentityReferenceError, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    PersistentSymbolError,
};
use scoop_wire::HashError;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalTypeDescriptorBuildError {
    Identity(HashError),
    Symbol(PersistentSymbolError),
    Definition(ObjectDefinitionIdentityError),
}

impl fmt::Display for ExternalTypeDescriptorBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot derive external TypeDescriptor: {self:?}")
    }
}

impl std::error::Error for ExternalTypeDescriptorBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Symbol(error) => Some(error),
            Self::Definition(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum ExternalTypeDescriptorDecodeError {
    Provider(IdentityReferenceError),
    Target(IdentityReferenceError),
    Build(ExternalTypeDescriptorBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    RecordMismatch,
}
impl fmt::Display for ExternalTypeDescriptorDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded external TypeDescriptor: {self:?}"
        )
    }
}
impl std::error::Error for ExternalTypeDescriptorDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(source) | Self::Target(source) => Some(source),
            Self::Build(source) => Some(source),
            Self::Encode(source) => Some(source),
            Self::RecordMismatch => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalTypeDescriptorValidationError {
    Build(ExternalTypeDescriptorBuildError),
    ContractMismatch,
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}
impl fmt::Display for ExternalTypeDescriptorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid external TypeDescriptor definition: {self:?}"
        )
    }
}
impl std::error::Error for ExternalTypeDescriptorValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Build(source) => Some(source),
            _ => None,
        }
    }
}
