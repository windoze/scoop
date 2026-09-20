use super::*;
use std::fmt;

#[derive(Debug)]
pub enum NominalConstructorBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Nominal(NominalSourceBindingError),
    Inventory,
    MissingKey(PersistentConstructorId),
    MissingSource(PersistentConstructorId),
    Origin(PersistentConstructorId),
    Contract {
        declaration: PersistentConstructorId,
        reason: String,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for Error {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl From<NominalSourceBindingError> for Error {
    fn from(error: NominalSourceBindingError) -> Self {
        match error {
            NominalSourceBindingError::Resource(error) => Self::Resource(error),
            other => Self::Nominal(other),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::Inventory => f.write_str("complete nominal constructor inventory mismatch"),
            Self::MissingKey(id) => write!(f, "constructor {id} has no artifact-owned source key"),
            Self::MissingSource(id) => write!(f, "missing nominal constructor source {id}"),
            Self::Origin(id) => write!(f, "constructor {id} differs from its foundation origin"),
            Self::Contract {
                declaration,
                reason,
            } => write!(f, "invalid constructor source {declaration}: {reason}"),
        }
    }
}
impl std::error::Error for Error {}
