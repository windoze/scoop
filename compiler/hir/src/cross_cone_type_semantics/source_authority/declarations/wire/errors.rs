use super::*;
use std::fmt;

#[derive(Debug)]
pub enum TypeDeclarationSourceResolutionError<E> {
    Resource(WireError),
    Required(ProtectedDeclarationResolutionError<E>),
    Inventory {
        field: u64,
        error: SourceInventoryError,
    },
    Constructors(NominalSourceConstructorResolutionError<E>),
    Properties(NominalSourcePropertyResolutionError<E>),
    Callables(NominalSourceCallableResolutionError<E>),
    Dispatch(InheritanceSourceCallableResolutionError<E>),
}
impl<E> TypeDeclarationSourceResolutionError<E> {
    pub(super) fn inventory(field: u64, error: SourceInventoryError) -> Self {
        match error {
            SourceInventoryError::Resource(error) => Self::Resource(error),
            error => Self::Inventory { field, error },
        }
    }
}
impl<E: fmt::Display> fmt::Display for TypeDeclarationSourceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Required(error) => write!(f, "declaration source field 1: {error}"),
            Self::Inventory { field, error } => {
                write!(f, "declaration source field {field}: {error}")
            }
            Self::Constructors(error) => write!(f, "declaration source field 3: {error}"),
            Self::Properties(error) => write!(f, "declaration source field 4: {error}"),
            Self::Callables(error) => write!(f, "declaration source field 5: {error}"),
            Self::Dispatch(error) => write!(f, "declaration source field 9: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeDeclarationSourceResolutionError<E> {}
