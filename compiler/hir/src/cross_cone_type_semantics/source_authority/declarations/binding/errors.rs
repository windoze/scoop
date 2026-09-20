use super::*;
use std::fmt;

#[derive(Debug)]
pub enum TypeDeclarationSourceBindingError {
    Resource(WireError),
    ProtectedInventory,
    Nominals(Box<NominalSourceBindingError>),
    Members(Box<NominalMemberBindingError>),
    Constructors(Box<NominalConstructorBindingError>),
    Parameters(Box<NominalParameterBindingError>),
    Dispatch(Box<InheritanceDispatchBindingError>),
    Slots(Box<InheritanceSlotSourceBindingError>),
    Joined(Box<NominalDispatchBindingError>),
}
impl From<WireError> for TypeDeclarationSourceBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalSourceBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: NominalSourceBindingError) -> Self {
        match error {
            NominalSourceBindingError::Resource(error) => Self::Resource(error),
            other => Self::Nominals(Box::new(other)),
        }
    }
}
impl From<NominalMemberBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: NominalMemberBindingError) -> Self {
        match error {
            NominalMemberBindingError::Resource(error) => Self::Resource(error),
            other => Self::Members(Box::new(other)),
        }
    }
}
impl From<NominalConstructorBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: NominalConstructorBindingError) -> Self {
        match error {
            NominalConstructorBindingError::Resource(error) => Self::Resource(error),
            other => Self::Constructors(Box::new(other)),
        }
    }
}
impl From<NominalParameterBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: NominalParameterBindingError) -> Self {
        match error {
            NominalParameterBindingError::Resource(error) => Self::Resource(error),
            other => Self::Parameters(Box::new(other)),
        }
    }
}
impl From<InheritanceDispatchBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: InheritanceDispatchBindingError) -> Self {
        match error {
            InheritanceDispatchBindingError::Resource(error) => Self::Resource(error),
            other => Self::Dispatch(Box::new(other)),
        }
    }
}
impl From<InheritanceSlotSourceBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: InheritanceSlotSourceBindingError) -> Self {
        match error {
            InheritanceSlotSourceBindingError::Resource(error) => Self::Resource(error),
            other => Self::Slots(Box::new(other)),
        }
    }
}
impl From<NominalDispatchBindingError> for TypeDeclarationSourceBindingError {
    fn from(error: NominalDispatchBindingError) -> Self {
        match error {
            NominalDispatchBindingError::Resource(error) => Self::Resource(error),
            other => Self::Joined(Box::new(other)),
        }
    }
}
impl fmt::Display for TypeDeclarationSourceBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::ProtectedInventory => f.write_str(
                "declaration source protected inventory differs from complete source contracts",
            ),
            Self::Nominals(error) => error.fmt(f),
            Self::Members(error) => error.fmt(f),
            Self::Constructors(error) => error.fmt(f),
            Self::Parameters(error) => error.fmt(f),
            Self::Dispatch(error) => error.fmt(f),
            Self::Slots(error) => error.fmt(f),
            Self::Joined(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TypeDeclarationSourceBindingError {}
