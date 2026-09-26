use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDeclarationTableError {
    CallableKind,
    Duplicate,
    NonCanonicalOrder,
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
        })
    }
}
impl std::error::Error for ProtectedDeclarationTableError {}

#[derive(Debug)]
pub enum ProtectedDeclarationResolutionError<E> {
    Resource(WireError),
    Foundation(E),
    Table(ProtectedDeclarationTableError),
}
impl<E: fmt::Display> fmt::Display for ProtectedDeclarationResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Table(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDeclarationResolutionError<E> {}
