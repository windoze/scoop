use super::*;
use std::fmt;

#[derive(Debug)]
pub enum NominalParameterBindingError {
    Resource(WireError),
    Constructor(Box<NominalConstructorBindingError>),
    Member(Box<NominalMemberBindingError>),
    Contract(Box<SourceParameterContractError>),
    NominalSourcesMismatch,
    Inventory,
    Identity(String),
    MissingProtocol(CallableTemplateOrigin),
    Declaration(CallableTemplateOrigin),
    Position {
        owner: CallableTemplateOrigin,
        position: u32,
    },
}
impl From<WireError> for NominalParameterBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalConstructorBindingError> for NominalParameterBindingError {
    fn from(error: NominalConstructorBindingError) -> Self {
        match error {
            NominalConstructorBindingError::Resource(e) => Self::Resource(e),
            other => Self::Constructor(Box::new(other)),
        }
    }
}
impl From<NominalMemberBindingError> for NominalParameterBindingError {
    fn from(error: NominalMemberBindingError) -> Self {
        match error {
            NominalMemberBindingError::Resource(e) => Self::Resource(e),
            other => Self::Member(Box::new(other)),
        }
    }
}
impl From<SourceParameterContractError> for NominalParameterBindingError {
    fn from(error: SourceParameterContractError) -> Self {
        match error {
            SourceParameterContractError::Resource(e) => Self::Resource(e),
            other => Self::Contract(Box::new(other)),
        }
    }
}
impl fmt::Display for NominalParameterBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Constructor(e) => e.fmt(f),
            Self::Member(e) => e.fmt(f),
            Self::Contract(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::NominalSourcesMismatch => {
                f.write_str("parameter sources require the same bound nominal sources")
            }
            Self::Inventory => {
                f.write_str("parameter sources disagree with the bound nominal inventory")
            }
            Self::MissingProtocol(owner) => {
                write!(f, "missing source parameter protocol {owner:?}")
            }
            Self::Declaration(owner) => write!(f, "invalid parameter protocol owner {owner:?}"),
            Self::Position { owner, position } => {
                write!(f, "missing source parameter {owner:?} at {position}")
            }
        }
    }
}
impl std::error::Error for NominalParameterBindingError {}
