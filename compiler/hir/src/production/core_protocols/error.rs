use std::fmt;

use super::*;

#[derive(Debug)]
pub enum CoreProtocolCallableBuildError {
    UnknownFunction(u32),
    UnknownClassConstructor(u32),
    NonSourceFunction(u32),
    MissingDefinitionOrigin(CoreProtocolCallableDefinitionV1),
    TooManyTypeParameters {
        definition: CoreProtocolCallableDefinitionV1,
    },
    TypeParameterCountMismatch {
        definition: CoreProtocolCallableDefinitionV1,
    },
    MissingReceiver {
        definition: CoreProtocolCallableDefinitionV1,
    },
    MissingSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    DuplicateSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    InvalidSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    InvalidSignatureType {
        definition: CoreProtocolCallableDefinitionV1,
        error: crate::HirSignatureTypeMappingError,
    },
    DuplicateSignatureMismatch {
        definition: CoreProtocolCallableDefinitionV1,
    },
    UnknownConstructorOwner {
        definition: CoreProtocolCallableDefinitionV1,
        owner: u32,
    },
    UnknownConstructorSelfApplication {
        definition: CoreProtocolCallableDefinitionV1,
    },
}

impl fmt::Display for CoreProtocolCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core protocol callable: {self:?}")
    }
}

impl std::error::Error for CoreProtocolCallableBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolSignatureReferenceError {
    UnknownSourceType([u8; 32]),
    UnknownGenericType([u8; 32]),
}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreProtocolCallableValidationError {
    UnknownFunction([u8; 32]),
    UnknownGenericFunction([u8; 32]),
    UnknownConstructor([u8; 32]),
    UnknownGeneratedCallable([u8; 32]),
    SignatureReference(CoreProtocolSignatureReferenceError),
}

impl fmt::Display for CoreProtocolCallableValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core protocol callable: {self:?}")
    }
}

impl std::error::Error for CoreProtocolCallableValidationError {}
