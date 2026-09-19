use crate::{InheritanceGraphError, ProtectedPropertySemanticError};
use std::fmt;

#[derive(Debug)]
pub enum NominalSupportPropertySemanticError<E> {
    Foundation(E),
    Resource(scoop_wire::WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(InheritanceGraphError<E>),
    Runtime(ProtectedPropertySemanticError<E>),
    Owner,
    ConstIdentity,
    ConstType,
}
impl<E: fmt::Display> fmt::Display for NominalSupportPropertySemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
            Self::Owner => f.write_str(
                "property support representation is invalid for its source nominal kind",
            ),
            Self::ConstIdentity => f.write_str(
                "const support identity or source origin disagrees with its declaration",
            ),
            Self::ConstType => {
                f.write_str("const support value kind differs from its canonical source type")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NominalSupportPropertySemanticError<E> {}
