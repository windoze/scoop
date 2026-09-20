use super::*;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceConstructorBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Inventory(&'static str),
    MissingOwner(PersistentExactTypeId),
    MissingKey(PersistentConstructorId),
    MissingSource(PersistentConstructorId),
    RepeatedOwner(PersistentConstructorId),
    Owner(PersistentConstructorId),
    ForeignDeclaration(PersistentConstructorId),
    Visibility(PersistentConstructorId),
    DefinitionOrigin(PersistentConstructorId),
    Signature(PersistentConstructorId),
    Access {
        declaration: PersistentConstructorId,
        reason: String,
    },
}

impl From<WireError> for InheritanceConstructorBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for InheritanceConstructorBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl fmt::Display for InheritanceConstructorBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Inventory(table) => write!(f, "constructor source inventory mismatch: {table}"),
            Self::MissingOwner(id) => write!(f, "missing constructor inventory owner {id}"),
            Self::MissingKey(id) => {
                write!(f, "constructor {id} is absent from the owning foundation")
            }
            Self::MissingSource(id) => write!(f, "missing constructor source {id}"),
            Self::RepeatedOwner(id) => {
                write!(f, "constructor {id} belongs to multiple source owners")
            }
            Self::Owner(id) => write!(f, "constructor {id} disagrees with its inventory owner"),
            Self::ForeignDeclaration(id) => {
                write!(f, "constructor {id} belongs to another provider")
            }
            Self::Visibility(id) => write!(
                f,
                "constructor {id} is outside the public/protected source inventory"
            ),
            Self::DefinitionOrigin(id) => write!(
                f,
                "constructor {id} differs from its foundation definition origin"
            ),
            Self::Signature(id) => write!(
                f,
                "constructor {id} differs from its source parameter or result shape"
            ),
            Self::Access {
                declaration,
                reason,
            } => write!(
                f,
                "invalid constructor source access for {declaration}: {reason}"
            ),
        }
    }
}
impl std::error::Error for InheritanceConstructorBindingError {}
