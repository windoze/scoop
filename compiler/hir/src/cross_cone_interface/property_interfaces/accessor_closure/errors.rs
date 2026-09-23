use super::*;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyAccessorClosureValidationError {
    Resource(scoop_wire::WireError),
    MissingSourceAccessor {
        property: PropertyDeclarationId,
        role: AccessorRole,
        accessor: PersistentPropertyAccessorId,
    },
    RestrictedGetterExported {
        property: PropertyDeclarationId,
        accessor: PersistentPropertyAccessorId,
    },
    OrphanSourceAccessor(PersistentPropertyAccessorId),
    Visibility {
        accessor: PersistentPropertyAccessorId,
        expected: crate::DeclaredVisibilityV1,
        actual: crate::DeclaredVisibilityV1,
    },
    SetterLookup {
        property: PropertyDeclarationId,
        accessor: PersistentPropertyAccessorId,
    },
    DuplicateAccessorClaim {
        accessor: PersistentPropertyAccessorId,
        first: PropertyDeclarationId,
        second: PropertyDeclarationId,
    },
    MissingPublicAccessor {
        property: PropertyDeclarationId,
        role: AccessorRole,
        accessor: PersistentPropertyAccessorId,
    },
    RestrictedSetterExported {
        property: PropertyDeclarationId,
        accessor: PersistentPropertyAccessorId,
    },
    OrphanPublicAccessor(PersistentPropertyAccessorId),
    Owner {
        accessor: PersistentPropertyAccessorId,
        expected: PublicDeclarationOwnerV1,
        actual: PublicDeclarationOwnerV1,
    },
    Receiver {
        accessor: PersistentPropertyAccessorId,
        expected: Option<Box<SignatureTypeKey>>,
        actual: Option<Box<SignatureTypeKey>>,
    },
    ParameterArity {
        accessor: PersistentPropertyAccessorId,
        expected: usize,
        actual: usize,
    },
    ParameterType {
        accessor: PersistentPropertyAccessorId,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Result {
        accessor: PersistentPropertyAccessorId,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Access {
        accessor: PersistentPropertyAccessorId,
        expected: PublicLookupAccessV1,
        actual: PublicLookupAccessV1,
    },
    Execution(PersistentPropertyAccessorId),
    Implementation {
        accessor: PersistentPropertyAccessorId,
        actual: CallableImplementationV1,
    },
    OperatorRole {
        accessor: PersistentPropertyAccessorId,
        actual: CallableOperatorRoleV1,
    },
    Infix(PersistentPropertyAccessorId),
    Modality {
        accessor: PersistentPropertyAccessorId,
        expected: CallableModalityV1,
        actual: CallableModalityV1,
    },
    RuntimeOnlyAbstractAccessors(PropertyDeclarationId),
}

impl fmt::Display for PropertyAccessorClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::MissingSourceAccessor {
                property,
                role,
                accessor,
            } => write!(
                formatter,
                "property {property:?} has no source {role:?} declaration {accessor}"
            ),
            Self::RestrictedGetterExported { property, accessor } => write!(
                formatter,
                "restricted property {property:?} exports getter {accessor}"
            ),
            Self::OrphanSourceAccessor(accessor) => write!(
                formatter,
                "source accessor {accessor} has no logical property"
            ),
            Self::Visibility {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "getter {accessor} visibility {actual:?} differs from property visibility {expected:?}"
            ),
            Self::SetterLookup { property, accessor } => write!(
                formatter,
                "property {property:?} setter lookup differs from accessor {accessor} visibility"
            ),
            Self::DuplicateAccessorClaim {
                accessor,
                first,
                second,
            } => write!(
                formatter,
                "property accessor {accessor} is claimed by both {first:?} and {second:?}"
            ),
            Self::MissingPublicAccessor {
                property,
                role,
                accessor,
            } => write!(
                formatter,
                "property {property:?} has no public {role:?} callable {accessor}"
            ),
            Self::RestrictedSetterExported { property, accessor } => write!(
                formatter,
                "property {property:?} exports restricted setter {accessor}"
            ),
            Self::OrphanPublicAccessor(accessor) => {
                write!(
                    formatter,
                    "public property accessor {accessor} has no property"
                )
            }
            Self::Owner {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} owner {actual:?} does not match {expected:?}"
            ),
            Self::Receiver {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} receiver {actual:?} does not match {expected:?}"
            ),
            Self::ParameterArity {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} expects {expected} parameters, found {actual}"
            ),
            Self::ParameterType {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} parameter type {actual:?} does not match {expected:?}"
            ),
            Self::Result {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} result {actual:?} does not match {expected:?}"
            ),
            Self::Access {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} access {actual:?} does not match {expected:?}"
            ),
            Self::Execution(accessor) => {
                write!(formatter, "property accessor {accessor} must be ordinary")
            }
            Self::Implementation { accessor, actual } => write!(
                formatter,
                "property accessor {accessor} must use Scoop implementation, found {actual:?}"
            ),
            Self::OperatorRole { accessor, actual } => write!(
                formatter,
                "property accessor {accessor} cannot have operator role {actual:?}"
            ),
            Self::Infix(accessor) => {
                write!(formatter, "property accessor {accessor} cannot be infix")
            }
            Self::Modality {
                accessor,
                expected,
                actual,
            } => write!(
                formatter,
                "property accessor {accessor} modality {actual:?} does not match {expected:?}"
            ),
            Self::RuntimeOnlyAbstractAccessors(property) => write!(
                formatter,
                "runtime property {property:?} has only abstract accessors"
            ),
        }
    }
}

impl std::error::Error for PropertyAccessorClosureValidationError {}
