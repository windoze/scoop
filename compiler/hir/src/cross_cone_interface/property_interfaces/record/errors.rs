use std::fmt;

use crate::{
    BinderListValidationError, PropertyCapabilityResolutionError, PropertyDeclarationId,
    PropertyPublicAccessV1, PublicDeclarationOwnerV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyInterfaceRecordBuildError {
    AccessorImplementations(PropertyDeclarationId),
    NonPublicDeclaration(crate::DeclaredVisibilityV1),
    MissingSetterLookup(PropertyDeclarationId),
    UnexpectedExtensionOwner(PropertyDeclarationId),
    ExtensionOwnerRequired {
        declaration: PropertyDeclarationId,
        actual: PublicDeclarationOwnerV1,
    },
    UnexpectedTypeParameters(PropertyDeclarationId),
    MissingExtensionReceiver,
    UnexpectedReceiver(PublicDeclarationOwnerV1),
    DirectPropertyContract {
        declaration: PropertyDeclarationId,
        owner: PublicDeclarationOwnerV1,
        access: PropertyPublicAccessV1,
    },
    ConstMustBeReadOnly(PropertyDeclarationId),
    ConstCannotBeGeneric(PropertyDeclarationId),
    ConstCannotHaveReceiver(PropertyDeclarationId),
    ConstMustBeDirect(PropertyDeclarationId),
    AbstractNominalOwnerRequired {
        declaration: PropertyDeclarationId,
        actual: PublicDeclarationOwnerV1,
    },
    AbstractSlotAccessRequired(PropertyDeclarationId),
}

impl fmt::Display for PropertyInterfaceRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessorImplementations(declaration) => write!(
                formatter,
                "property {declaration:?} accessor implementations disagree with its source representation"
            ),
            Self::NonPublicDeclaration(visibility) => write!(
                formatter,
                "property declaration with visibility {visibility:?} cannot enter public lookup"
            ),
            Self::MissingSetterLookup(declaration) => write!(
                formatter,
                "read-only property {declaration:?} cannot expose a setter"
            ),
            Self::UnexpectedExtensionOwner(declaration) => write!(
                formatter,
                "ordinary property {declaration:?} cannot have an extension owner"
            ),
            Self::ExtensionOwnerRequired {
                declaration,
                actual,
            } => write!(
                formatter,
                "extension property {declaration:?} requires an extension owner, found {actual:?}"
            ),
            Self::UnexpectedTypeParameters(declaration) => write!(
                formatter,
                "ordinary property {declaration:?} cannot declare type parameters"
            ),
            Self::MissingExtensionReceiver => {
                formatter.write_str("extension property has no receiver")
            }
            Self::UnexpectedReceiver(owner) => {
                write!(
                    formatter,
                    "{owner:?} property cannot declare an extension receiver"
                )
            }
            Self::DirectPropertyContract {
                declaration,
                owner,
                access,
            } => write!(
                formatter,
                "property {declaration:?} owned by {owner:?} requires direct-only access, found {access:?}"
            ),
            Self::ConstMustBeReadOnly(declaration) => {
                write!(
                    formatter,
                    "const property {declaration:?} must be read-only"
                )
            }
            Self::ConstCannotBeGeneric(declaration) => {
                write!(
                    formatter,
                    "const property {declaration:?} cannot be generic"
                )
            }
            Self::ConstCannotHaveReceiver(declaration) => write!(
                formatter,
                "const property {declaration:?} cannot have an extension receiver"
            ),
            Self::ConstMustBeDirect(declaration) => write!(
                formatter,
                "const property {declaration:?} must use direct-only access"
            ),
            Self::AbstractNominalOwnerRequired {
                declaration,
                actual,
            } => write!(
                formatter,
                "abstract property {declaration:?} requires a nominal owner, found {actual:?}"
            ),
            Self::AbstractSlotAccessRequired(declaration) => write!(
                formatter,
                "abstract property {declaration:?} requires public slot access"
            ),
        }
    }
}

impl std::error::Error for PropertyInterfaceRecordBuildError {}

#[derive(Debug)]
pub enum PropertyInterfaceRecordResolutionError<E> {
    Declaration(E),
    Owner(E),
    TypeParameters(BinderListValidationError<E>),
    Receiver(E),
    ValueType(E),
    Capability(PropertyCapabilityResolutionError<E>),
    Record(PropertyInterfaceRecordBuildError),
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid property declaration: {error}"),
            Self::Owner(error) => write!(formatter, "invalid property owner: {error}"),
            Self::TypeParameters(error) => {
                write!(formatter, "invalid property type parameters: {error}")
            }
            Self::Receiver(error) => write!(formatter, "invalid property receiver: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid property value type: {error}"),
            Self::Capability(error) => write!(formatter, "invalid property capability: {error}"),
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for PropertyInterfaceRecordResolutionError<E>
{
}
