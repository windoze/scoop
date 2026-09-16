use std::fmt;

use scoop_identity::{PersistentConstructorId, PersistentEnumVariantId, PersistentExportBindingId};

use super::{
    BinderListValidationError, CanonicalPersistentIdSetValidationError,
    NominalSourceShapeResolutionError, PublicMemberRefSetValidationError, PublicNominalKindV1,
    SignatureTypeSetValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalInterfaceRecordBuildError {
    SourceShapeKind {
        expected: PublicNominalKindV1,
        actual: PublicNominalKindV1,
    },
    ConstructorsNotAllowed(PublicNominalKindV1),
    ConstructorMember(PersistentConstructorId),
    VariantConstructorMember(PersistentEnumVariantId),
}

impl fmt::Display for NominalInterfaceRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceShapeKind { expected, actual } => write!(
                formatter,
                "source shape kind {actual:?} does not match nominal kind {expected:?}"
            ),
            Self::ConstructorsNotAllowed(kind) => {
                write!(formatter, "{kind:?} cannot declare source constructors")
            }
            Self::ConstructorMember(constructor) => write!(
                formatter,
                "source constructor {constructor} must not appear in the ordinary member set"
            ),
            Self::VariantConstructorMember(variant) => write!(
                formatter,
                "enum variant constructor {variant} must not appear in the ordinary member set"
            ),
        }
    }
}

impl std::error::Error for NominalInterfaceRecordBuildError {}

#[derive(Debug)]
pub enum NominalInterfaceRecordResolutionError<E> {
    Declaration(E),
    TypeParameters(BinderListValidationError<E>),
    ExactSupertypes(SignatureTypeSetValidationError<E>),
    Constructors(CanonicalPersistentIdSetValidationError<PersistentConstructorId, E>),
    Members(PublicMemberRefSetValidationError<E>),
    NestedBindings(CanonicalPersistentIdSetValidationError<PersistentExportBindingId, E>),
    SourceShape(NominalSourceShapeResolutionError<E>),
    Record(NominalInterfaceRecordBuildError),
}

impl<E: fmt::Display> fmt::Display for NominalInterfaceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid nominal declaration: {error}"),
            Self::TypeParameters(error) => {
                write!(formatter, "invalid nominal type parameters: {error}")
            }
            Self::ExactSupertypes(error) => {
                write!(formatter, "invalid nominal exact supertypes: {error}")
            }
            Self::Constructors(error) => {
                write!(formatter, "invalid nominal constructors: {error}")
            }
            Self::Members(error) => write!(formatter, "invalid nominal members: {error}"),
            Self::NestedBindings(error) => {
                write!(formatter, "invalid nominal nested bindings: {error}")
            }
            Self::SourceShape(error) => write!(formatter, "invalid nominal source shape: {error}"),
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for NominalInterfaceRecordResolutionError<E>
{
}
