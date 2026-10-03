use scoop_identity::{
    IdentityReferenceError, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentSymbolError, PersistentSymbolRequest,
    ScoopAbiResolutionError,
};
use scoop_wire::HashError;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableLinkContractError {
    Identity(HashError),
    Symbol(PersistentSymbolError),
    Definition(ObjectDefinitionIdentityError),
}

impl fmt::Display for CallableLinkContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(source) => {
                write!(formatter, "cannot derive callable identity: {source}")
            }
            Self::Symbol(source) => write!(formatter, "cannot derive callable symbol: {source}"),
            Self::Definition(source) => {
                write!(formatter, "cannot derive callable definition: {source}")
            }
        }
    }
}

impl std::error::Error for CallableLinkContractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::Symbol(source) => Some(source),
            Self::Definition(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableAbiBuildError {
    Suspend,
    RootProtocolMismatch {
        abi: scoop_identity::GcEffect,
        root: scoop_identity::GcEffect,
    },
    Contract(CallableLinkContractError),
}
impl fmt::Display for CallableAbiBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid callable ABI record: {self:?}")
    }
}
impl std::error::Error for CallableAbiBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum CallableAbiDecodeError {
    Target(IdentityReferenceError),
    Abi(ScoopAbiResolutionError<IdentityReferenceError>),
    Build(CallableAbiBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    RecordMismatch,
}
impl fmt::Display for CallableAbiDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid decoded callable ABI record: {self:?}")
    }
}
impl std::error::Error for CallableAbiDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::Abi(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::RecordMismatch => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableAbiValidationError {
    Contract(CallableLinkContractError),
    ContractMismatch,
    MissingBody(PersistentCallableBodyId),
    MissingSymbol(PersistentSymbolRequest),
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}
impl fmt::Display for CallableAbiValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid callable ABI definition: {self:?}")
    }
}
impl std::error::Error for CallableAbiValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}
