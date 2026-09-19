use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDeclarationTableError {
    CallableKind,
    Duplicate,
    NonCanonicalOrder,
    Inventory,
    AccessorClosure,
}
impl fmt::Display for ProtectedDeclarationTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CallableKind => {
                "protected callable reference requires a function, generic function or accessor"
            }
            Self::Duplicate => "duplicate protected declaration identity",
            Self::NonCanonicalOrder => {
                "protected declaration identities are not in canonical order"
            }
            Self::Inventory => {
                "protected declaration table differs from the complete definition-side inventory"
            }
            Self::AccessorClosure => {
                "protected property is missing its declared protected accessor"
            }
        })
    }
}
impl std::error::Error for ProtectedDeclarationTableError {}

#[derive(Debug)]
pub enum ProtectedDeclarationResolutionError<E> {
    Resource(WireError),
    Foundation(E),
    Table(ProtectedDeclarationTableError),
    Callable(ProtectedCallableInterfaceResolutionError<E>),
    Property(ProtectedPropertyResolutionError<E>),
    Nested(NestedSourceResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for ProtectedDeclarationResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Table(error) => error.fmt(f),
            Self::Callable(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
            Self::Nested(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDeclarationResolutionError<E> {}

#[derive(Debug)]
pub enum ProtectedDeclarationSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    Table(ProtectedDeclarationTableError),
    Callable(ProtectedCallableSemanticError<E>),
    Property(ProtectedPropertySemanticError<E>),
    Nested(NestedSourceSemanticError<E>),
    Accessor(ProtectedPropertyAccessorClosureError),
}
impl<E: fmt::Display> fmt::Display for ProtectedDeclarationSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Table(error) => error.fmt(f),
            Self::Callable(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
            Self::Nested(error) => error.fmt(f),
            Self::Accessor(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDeclarationSemanticError<E> {}
