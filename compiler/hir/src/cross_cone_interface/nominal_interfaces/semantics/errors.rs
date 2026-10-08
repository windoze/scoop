use std::fmt;

use scoop_identity::{PersistentConstructorId, PersistentExportBindingId, SourceDeclarationKind};

use crate::{
    NominalSignatureSemanticError, NominalSourceShapeSemanticError, PublicDeclarationOwnerV1,
    PublicMemberRefV1, PublicNominalKindV1, SourceNominalId,
    TypeParameterBinderSemanticValidationError,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ExactSupertypeSemanticError<E> {
    Signature(NominalSignatureSemanticError<E>),
    InvalidTargetKind(PublicNominalKindV1),
    ClassNotAllowed { owner: PublicNominalKindV1 },
    MultipleClasses { first_index: usize },
}

#[derive(Debug)]
pub enum NominalInterfaceSemanticValidationError<E> {
    Declaration(E),
    ClassDispatchRole(PublicNominalKindV1),
    DispatchReceiver {
        index: usize,
        error: crate::SignatureTypeSemanticError<E>,
    },
    DispatchApplication {
        index: usize,
        error: ExactSupertypeSemanticError<E>,
    },
    DeclarationKind {
        expected: PublicNominalKindV1,
        actual: SourceDeclarationKind,
    },
    TypeParameterArity {
        expected: u32,
        actual: u32,
    },
    ObjectTypeParameters {
        actual: u32,
    },
    TypeParameters(TypeParameterBinderSemanticValidationError<E>),
    ExactSupertype {
        index: usize,
        error: ExactSupertypeSemanticError<E>,
    },
    ConstructorReference {
        index: usize,
        error: E,
    },
    ConstructorOwner {
        index: usize,
        constructor: PersistentConstructorId,
        expected: SourceNominalId,
        actual: PublicDeclarationOwnerV1,
    },
    MemberReference {
        index: usize,
        error: E,
    },
    MemberOwner {
        index: usize,
        member: PublicMemberRefV1,
        expected: SourceNominalId,
        actual: PublicDeclarationOwnerV1,
    },
    NestedBindingReference {
        index: usize,
        error: E,
    },
    NestedBindingOwner {
        index: usize,
        binding: PersistentExportBindingId,
        expected: SourceNominalId,
        actual: PublicDeclarationOwnerV1,
    },
    SourceShape(NominalSourceShapeSemanticError<E>),
}

impl<E: fmt::Display> fmt::Display for ExactSupertypeSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature(error) => error.fmt(formatter),
            Self::InvalidTargetKind(kind) => {
                write!(formatter, "{kind:?} cannot be used as a supertype")
            }
            Self::ClassNotAllowed { owner } => {
                write!(formatter, "{owner:?} cannot declare a class supertype")
            }
            Self::MultipleClasses { first_index } => write!(
                formatter,
                "multiple class supertypes; the first class is at index {first_index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExactSupertypeSemanticError<E> {}

impl<E: fmt::Display> fmt::Display for NominalInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid nominal declaration: {error}"),
            Self::DispatchReceiver { index, error } => {
                write!(formatter, "invalid dispatch receiver {index}: {error}")
            }
            Self::ClassDispatchRole(kind) => write!(
                formatter,
                "{kind:?} cannot declare a class vtable selection"
            ),
            Self::DispatchApplication { index, error } => write!(
                formatter,
                "invalid dispatch interface application {index}: {error}"
            ),
            Self::DeclarationKind { expected, actual } => write!(
                formatter,
                "nominal declaration kind {actual:?} does not match {expected:?}"
            ),
            Self::TypeParameterArity { expected, actual } => write!(
                formatter,
                "nominal declaration expects {expected} type parameters, found {actual}"
            ),
            Self::ObjectTypeParameters { actual } => {
                write!(formatter, "object declaration has {actual} type parameters")
            }
            Self::TypeParameters(error) => {
                write!(formatter, "invalid nominal type parameters: {error}")
            }
            Self::ExactSupertype { index, error } => {
                write!(formatter, "invalid exact supertype {index}: {error}")
            }
            Self::ConstructorReference { index, error } => {
                write!(formatter, "invalid constructor {index}: {error}")
            }
            Self::ConstructorOwner {
                index,
                constructor,
                expected,
                actual,
            } => write!(
                formatter,
                "constructor {constructor} at index {index} has owner {actual:?}, expected nominal {expected:?}"
            ),
            Self::MemberReference { index, error } => {
                write!(formatter, "invalid member {index}: {error}")
            }
            Self::MemberOwner {
                index,
                member,
                expected,
                actual,
            } => write!(
                formatter,
                "member {member:?} at index {index} has owner {actual:?}, expected nominal {expected:?}"
            ),
            Self::NestedBindingReference { index, error } => {
                write!(formatter, "invalid nested binding {index}: {error}")
            }
            Self::NestedBindingOwner {
                index,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "nested binding {binding} at index {index} has owner {actual:?}, expected nominal {expected:?}"
            ),
            Self::SourceShape(error) => write!(formatter, "invalid nominal source shape: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for NominalInterfaceSemanticValidationError<E>
{
}
