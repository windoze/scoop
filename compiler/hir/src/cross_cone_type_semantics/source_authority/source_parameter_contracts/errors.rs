use super::*;
use std::fmt;

#[derive(Debug)]
pub enum SourceParameterContractError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Arity(CallableTemplateOrigin),
    Shape {
        owner: CallableTemplateOrigin,
        position: u32,
    },
    Vararg {
        owner: CallableTemplateOrigin,
        position: u32,
    },
    Origin {
        owner: CallableTemplateOrigin,
        position: u32,
    },
}
impl From<WireError> for SourceParameterContractError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for SourceParameterContractError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl fmt::Display for SourceParameterContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Arity(owner) => write!(f, "parameter protocol arity differs for {owner:?}"),
            Self::Shape { owner, position } => write!(
                f,
                "source parameter shape differs for {owner:?} at {position}"
            ),
            Self::Vararg { owner, position } => write!(
                f,
                "source vararg is not canonical Array for {owner:?} at {position}"
            ),
            Self::Origin { owner, position } => write!(
                f,
                "source parameter origin differs for {owner:?} at {position}"
            ),
        }
    }
}
impl std::error::Error for SourceParameterContractError {}

impl From<SourceParameterContractError> for InheritanceParameterBindingError {
    fn from(error: SourceParameterContractError) -> Self {
        match error {
            SourceParameterContractError::Resource(e) => Self::Resource(e),
            SourceParameterContractError::Foundation(e) => Self::Foundation(e),
            SourceParameterContractError::Arity(owner) => Self::Arity(owner),
            SourceParameterContractError::Shape { owner, position } => {
                Self::Shape { owner, position }
            }
            SourceParameterContractError::Vararg { owner, position } => {
                Self::Vararg { owner, position }
            }
            SourceParameterContractError::Origin { owner, position } => {
                Self::Origin { owner, position }
            }
        }
    }
}
