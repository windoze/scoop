use super::*;
use scoop_identity::DefinitionOriginSubject;
use std::fmt;

mod semantics;

#[derive(Debug)]
pub enum NominalMemberBindingError {
    Resource(WireError),
    Foundation(Box<TypeFoundationBindingError>),
    Nominal(Box<NominalSourceBindingError>),
    Inheritance(InheritanceGraphError<TypeFoundationBindingError>),
    Callable(Box<NominalSupportCallableSemanticError<Self>>),
    Property(Box<NominalSupportPropertySemanticError<Self>>),
    Accessor(ProtectedPropertyAccessorClosureError),
    Inventory(&'static str),
    MissingProperty(PersistentPropertyId),
    MissingCallableKey(CallableTemplateOrigin),
    MissingCallable(CallableTemplateOrigin),
    Origin(DefinitionOriginSubject),
    Identity(String),
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for Error {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(e) => Self::Resource(e),
            other => Self::Foundation(Box::new(other)),
        }
    }
}
impl From<NominalSourceBindingError> for Error {
    fn from(error: NominalSourceBindingError) -> Self {
        match error {
            NominalSourceBindingError::Resource(e) => Self::Resource(e),
            other => Self::Nominal(Box::new(other)),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Nominal(e) => e.fmt(f),
            Self::Inheritance(e) => e.fmt(f),
            Self::Callable(e) => e.fmt(f),
            Self::Property(e) => e.fmt(f),
            Self::Accessor(e) => e.fmt(f),
            Self::Inventory(table) => {
                write!(f, "nominal member source inventory mismatch: {table}")
            }
            Self::MissingProperty(id) => {
                write!(f, "missing nominal property source or owned key {id}")
            }
            Self::MissingCallableKey(id) => {
                write!(f, "no artifact-owned declaration key for callable {id:?}")
            }
            Self::MissingCallable(id) => write!(f, "missing nominal callable source {id:?}"),
            Self::Origin(subject) => write!(
                f,
                "nominal member source differs from foundation origin {subject:?}"
            ),
            Self::Identity(e) => write!(f, "invalid nominal member identity: {e}"),
        }
    }
}
impl std::error::Error for Error {}
