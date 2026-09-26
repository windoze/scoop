use crate::{
    InheritanceGraphError, NominalSourceShapeSemanticError, NominalSupportCallableSemanticError,
    NominalSupportPropertySemanticError, ProtectedPropertyAccessorClosureError,
    SignatureTypeSemanticError, TypeParameterBinderSemanticValidationError,
};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum NestedSourceSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(InheritanceGraphError<E>),
    Signature(SignatureTypeSemanticError<E>),
    Binders(TypeParameterBinderSemanticValidationError<E>),
    Shape(NominalSourceShapeSemanticError<E>),
    Callable(NominalSupportCallableSemanticError<E>),
    Property(NominalSupportPropertySemanticError<E>),
    Accessor(ProtectedPropertyAccessorClosureError),
    Owner,
    Identity,
    Modality,
    Inventory,
    Supertypes,
    ReferenceClosure,
}
impl<E: fmt::Display> fmt::Display for NestedSourceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Binders(error) => error.fmt(f),
            Self::Shape(error) => error.fmt(f),
            Self::Callable(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
            Self::Accessor(error) => error.fmt(f),
            Self::Owner => f.write_str("nested source has no canonical nominal owner"),
            Self::Identity => {
                f.write_str("nested nominal source identity, kind, binder arity or access differs")
            }
            Self::Modality => {
                f.write_str("nested nominal modality differs from its source declaration")
            }
            Self::Inventory => f.write_str(
                "nested source metadata differs from the complete declaration-side inventory",
            ),
            Self::Supertypes => {
                f.write_str("nested nominal has an invalid class/interface supertype partition")
            }
            Self::ReferenceClosure => {
                f.write_str("nested support source reference closure is incomplete")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NestedSourceSemanticError<E> {}
