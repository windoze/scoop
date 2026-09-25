use super::*;
use crate::{
    BinderListValidationError, DeclarationAccessSourceResolutionError,
    NominalSourceShapeResolutionError, NominalSupportPropertyResolutionError,
    ProtectedCallableInterfaceResolutionError, SignatureTypeSetValidationError,
};
use std::fmt;

#[derive(Debug)]
pub enum NestedSourceResolutionError<E> {
    Resource(WireError),
    Foundation(E),
    Binders(BinderListValidationError<E>),
    Supertypes(SignatureTypeSetValidationError<E>),
    Shape(NominalSourceShapeResolutionError<E>),
    Access(DeclarationAccessSourceResolutionError<E>),
    Callable(ProtectedCallableInterfaceResolutionError<E>),
    Property(NominalSupportPropertyResolutionError<E>),
    Build(NestedSourceBuildError),
}
impl<E: fmt::Display> fmt::Display for NestedSourceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Binders(error) => error.fmt(f),
            Self::Supertypes(error) => error.fmt(f),
            Self::Shape(error) => error.fmt(f),
            Self::Access(error) => error.fmt(f),
            Self::Callable(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NestedSourceResolutionError<E> {}
