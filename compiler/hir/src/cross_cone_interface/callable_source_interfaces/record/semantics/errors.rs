use std::fmt;

use scoop_identity::{CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, SignatureTypeKey};

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceSemanticValidationError<E> {
    CallableDeclaration {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    ParameterArity {
        expected: usize,
        actual: usize,
    },
    ParameterName {
        index: usize,
        expected: CanonicalIdentifier,
        actual: CanonicalIdentifier,
    },
    ParameterType {
        index: usize,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    ArrayDeclaration {
        index: usize,
        error: E,
    },
    VarargArrayType {
        index: usize,
        element_type: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    DefinitionOriginCone {
        index: usize,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DefinitionOrigin {
        index: usize,
        error: E,
    },
}

impl<E: fmt::Display> fmt::Display for CallableSourceInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableDeclaration { expected, actual } => write!(
                formatter,
                "source interface owner {expected:?} does not match callable interface {actual:?}"
            ),
            Self::ParameterArity { expected, actual } => write!(
                formatter,
                "callable interface has {expected} parameters, source interface has {actual}"
            ),
            Self::ParameterName {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {index} has name {actual:?}, expected {expected:?}"
            ),
            Self::ParameterType {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {index} has type {actual:?}, expected {expected:?}"
            ),
            Self::ArrayDeclaration { index, error } => write!(
                formatter,
                "invalid intrinsic Array declaration for source parameter {index}: {error}"
            ),
            Self::VarargArrayType {
                index,
                element_type,
                actual,
            } => write!(
                formatter,
                "vararg source parameter {index} has value type {actual:?}, expected Array of {element_type:?}"
            ),
            Self::DefinitionOriginCone {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {index} definition origin belongs to Cone {actual}, expected {expected}"
            ),
            Self::DefinitionOrigin { index, error } => write!(
                formatter,
                "invalid definition origin for source parameter {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableSourceInterfaceSemanticValidationError<E>
{
}
