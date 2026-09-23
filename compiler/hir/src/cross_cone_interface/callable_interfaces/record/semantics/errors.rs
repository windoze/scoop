use std::fmt;

use scoop_identity::SignatureTypeKey;

use crate::{
    PublicDeclarationOwnerV1, SignatureTypeSemanticError,
    TypeParameterBinderSemanticValidationError,
};

#[derive(Debug, Eq, PartialEq)]
pub enum CallableInterfaceSemanticValidationError<E> {
    Declaration(E),
    Owner {
        expected: PublicDeclarationOwnerV1,
        actual: PublicDeclarationOwnerV1,
    },
    TypeParameterArity {
        expected: u32,
        actual: u32,
    },
    ReceiverMismatch {
        expected: Option<Box<SignatureTypeKey>>,
        actual: Option<Box<SignatureTypeKey>>,
    },
    ParameterArity {
        expected: usize,
        actual: usize,
    },
    ParameterTypeMismatch {
        index: usize,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    ConstructedType {
        owner: PublicDeclarationOwnerV1,
        actual: Box<SignatureTypeKey>,
    },
    TypeParameters(TypeParameterBinderSemanticValidationError<E>),
    Receiver(SignatureTypeSemanticError<E>),
    Parameter {
        index: usize,
        error: SignatureTypeSemanticError<E>,
    },
    Result(SignatureTypeSemanticError<E>),
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid callable declaration: {error}"),
            Self::Owner { expected, actual } => write!(
                formatter,
                "callable owner {actual:?} does not match identity owner {expected:?}"
            ),
            Self::TypeParameterArity { expected, actual } => write!(
                formatter,
                "callable identity expects {expected} type parameters, found {actual}"
            ),
            Self::ReceiverMismatch { expected, actual } => write!(
                formatter,
                "callable receiver {actual:?} does not match identity receiver {expected:?}"
            ),
            Self::ParameterArity { expected, actual } => write!(
                formatter,
                "callable identity expects {expected} parameters, found {actual}"
            ),
            Self::ParameterTypeMismatch {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "callable parameter {index} has type {actual:?}, expected identity type {expected:?}"
            ),
            Self::ConstructedType { owner, actual } => write!(
                formatter,
                "constructor result {actual:?} does not preserve its declared nominal owner {owner:?}"
            ),
            Self::TypeParameters(error) => {
                write!(formatter, "invalid callable type parameters: {error}")
            }
            Self::Receiver(error) => write!(formatter, "invalid callable receiver: {error}"),
            Self::Parameter { index, error } => {
                write!(formatter, "invalid callable parameter {index}: {error}")
            }
            Self::Result(error) => write!(formatter, "invalid callable result: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableInterfaceSemanticValidationError<E>
{
}
