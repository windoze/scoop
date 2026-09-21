use super::*;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceParameterBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Constructor(InheritanceConstructorBindingError),
    Callable(Box<InheritanceProtectedCallableBindingError>),
    FoundationMismatch,
    Inventory,
    Identity(String),
    MissingProtocol(CallableTemplateOrigin),
    Declaration(CallableTemplateOrigin),
    Arity(CallableTemplateOrigin),
    Position {
        owner: CallableTemplateOrigin,
        position: u32,
    },
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
impl From<WireError> for InheritanceParameterBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for InheritanceParameterBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl From<InheritanceConstructorBindingError> for InheritanceParameterBindingError {
    fn from(error: InheritanceConstructorBindingError) -> Self {
        match error {
            InheritanceConstructorBindingError::Resource(error) => Self::Resource(error),
            other => Self::Constructor(other),
        }
    }
}
impl From<InheritanceProtectedCallableBindingError> for InheritanceParameterBindingError {
    fn from(error: InheritanceProtectedCallableBindingError) -> Self {
        match error {
            InheritanceProtectedCallableBindingError::Resource(error) => Self::Resource(error),
            other => Self::Callable(Box::new(other)),
        }
    }
}
impl fmt::Display for InheritanceParameterBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Constructor(e) => e.fmt(f),
            Self::Callable(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::FoundationMismatch => {
                f.write_str("parameter sources require the same bound foundation")
            }
            Self::Inventory => {
                f.write_str("parameter sources disagree with the bound inheritance inventory")
            }
            Self::MissingProtocol(owner) => {
                write!(f, "missing source parameter protocol {owner:?}")
            }
            Self::Declaration(owner) => write!(f, "invalid parameter protocol owner {owner:?}"),
            Self::Arity(owner) => write!(f, "parameter protocol arity differs for {owner:?}"),
            Self::Position { owner, position } => {
                write!(f, "missing source parameter {owner:?} at {position}")
            }
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
impl std::error::Error for InheritanceParameterBindingError {}
