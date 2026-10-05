use std::fmt;

use scoop_identity::{AccessorRole, PersistentPropertyAccessorId, SignatureTypeKey};

use crate::{
    PropertyCapabilityV1, PropertyDeclarationId, PropertyPublicAccessV1, PropertyRepresentationV1,
    PublicDeclarationOwnerV1, PublicNominalKindV1, SignatureTypeSemanticError,
    TypeParameterBinderSemanticValidationError,
};

#[derive(Debug, Eq, PartialEq)]
pub enum PropertyInterfaceSemanticValidationError<E> {
    Declaration(E),
    Source(E),
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
    TypeParameters(TypeParameterBinderSemanticValidationError<E>),
    Receiver(SignatureTypeSemanticError<E>),
    ValueType(SignatureTypeSemanticError<E>),
    AccessorReference {
        accessor: PersistentPropertyAccessorId,
        expected_role: AccessorRole,
        error: E,
    },
    AccessorOwner {
        accessor: PersistentPropertyAccessorId,
        expected: PropertyDeclarationId,
        actual: PropertyDeclarationId,
    },
    AccessorRole {
        accessor: PersistentPropertyAccessorId,
        expected: AccessorRole,
        actual: AccessorRole,
    },
    Capability {
        expected: Box<PropertyCapabilityV1>,
        actual: Box<PropertyCapabilityV1>,
    },
    Representation {
        expected: PropertyRepresentationV1,
        actual: PropertyRepresentationV1,
    },
    Access {
        expected: PropertyPublicAccessV1,
        actual: PropertyPublicAccessV1,
    },
    ConstOwner(E),
    ConstOwnerKind {
        actual: PublicNominalKindV1,
    },
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid property declaration: {error}"),
            Self::Source(error) => write!(formatter, "invalid property source shape: {error}"),
            Self::Owner { expected, actual } => write!(
                formatter,
                "property owner {actual:?} does not match identity owner {expected:?}"
            ),
            Self::TypeParameterArity { expected, actual } => write!(
                formatter,
                "property identity expects {expected} type parameters, found {actual}"
            ),
            Self::ReceiverMismatch { expected, actual } => write!(
                formatter,
                "property receiver {actual:?} does not match identity receiver {expected:?}"
            ),
            Self::TypeParameters(error) => {
                write!(formatter, "invalid property type parameters: {error}")
            }
            Self::Receiver(error) => write!(formatter, "invalid property receiver: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid property value type: {error}"),
            Self::AccessorReference {
                accessor,
                expected_role,
                error,
            } => write!(
                formatter,
                "invalid {expected_role:?} property accessor {accessor}: {error}"
            ),
            Self::AccessorOwner {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} owner {actual:?} does not match {expected:?}"
            ),
            Self::AccessorRole {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} has role {actual:?}, expected {expected:?}"
            ),
            Self::Capability { expected, actual } => write!(
                formatter,
                "property capability {actual:?} does not match source {expected:?}"
            ),
            Self::Representation { expected, actual } => write!(
                formatter,
                "property representation {actual:?} does not match source {expected:?}"
            ),
            Self::Access { expected, actual } => write!(
                formatter,
                "property access {actual:?} does not match source {expected:?}"
            ),
            Self::ConstOwner(error) => write!(formatter, "invalid const property owner: {error}"),
            Self::ConstOwnerKind { actual } => write!(
                formatter,
                "const member property requires an object owner, found {actual:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for PropertyInterfaceSemanticValidationError<E>
{
}
