use std::fmt;

use scoop_identity::{CanonicalIdentifierError, FieldIdentityError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFieldIdentityLocation {
    Struct { structure: u32, field: u32 },
    ClassField { field: u32 },
}

#[derive(Debug)]
pub enum HirFieldIdentityError {
    GeneratedStructOwner {
        structure: u32,
    },
    TooManyStructFields {
        structure: u32,
    },
    InvalidStructFieldName {
        structure: u32,
        field: u32,
        error: CanonicalIdentifierError,
    },
    InvalidStructFieldIdentity {
        structure: u32,
        field: u32,
        error: FieldIdentityError,
    },
    UnknownClassField {
        class: u32,
        field: u32,
    },
    DuplicateClassField {
        field: u32,
    },
    ClassFieldOwner {
        class: u32,
        field: u32,
        actual_class: u32,
    },
    UnknownProperty {
        field: u32,
        property: u32,
    },
    ExtensionPropertyField {
        field: u32,
        property: u32,
    },
    PropertyStorageMismatch {
        field: u32,
        property: u32,
    },
    UnknownDelegateStorage {
        field: u32,
        property: u32,
        storage: u32,
    },
    DelegateStorageMismatch {
        field: u32,
        property: u32,
    },
    SourceClassPropertyOwner {
        class: u32,
        field: u32,
        property: u32,
    },
    UnsupportedGeneratedClassOwner {
        class: u32,
        field: u32,
    },
    ObjectBackingPropertyOwner {
        class: u32,
        field: u32,
        property: u32,
    },
    UnknownObject {
        class: u32,
        field: u32,
        object: u32,
    },
    ObjectBackingRelation {
        class: u32,
        field: u32,
        object: u32,
    },
    InvalidClassFieldIdentity {
        class: u32,
        field: u32,
        error: FieldIdentityError,
    },
    UnownedClassField {
        field: u32,
    },
    DuplicatePersistentIdentity {
        location: HirFieldIdentityLocation,
    },
}

impl HirFieldIdentityError {
    pub const fn structure(&self) -> Option<u32> {
        match self {
            Self::GeneratedStructOwner { structure }
            | Self::TooManyStructFields { structure }
            | Self::InvalidStructFieldName { structure, .. }
            | Self::InvalidStructFieldIdentity { structure, .. }
            | Self::DuplicatePersistentIdentity {
                location: HirFieldIdentityLocation::Struct { structure, .. },
            } => Some(*structure),
            _ => None,
        }
    }

    pub const fn class(&self) -> Option<u32> {
        match self {
            Self::UnknownClassField { class, .. }
            | Self::ClassFieldOwner { class, .. }
            | Self::SourceClassPropertyOwner { class, .. }
            | Self::UnsupportedGeneratedClassOwner { class, .. }
            | Self::ObjectBackingPropertyOwner { class, .. }
            | Self::UnknownObject { class, .. }
            | Self::ObjectBackingRelation { class, .. }
            | Self::InvalidClassFieldIdentity { class, .. } => Some(*class),
            _ => None,
        }
    }

    pub const fn class_field(&self) -> Option<u32> {
        match self {
            Self::DuplicateClassField { field }
            | Self::UnknownProperty { field, .. }
            | Self::ExtensionPropertyField { field, .. }
            | Self::PropertyStorageMismatch { field, .. }
            | Self::UnknownDelegateStorage { field, .. }
            | Self::DelegateStorageMismatch { field, .. }
            | Self::UnownedClassField { field }
            | Self::DuplicatePersistentIdentity {
                location: HirFieldIdentityLocation::ClassField { field },
            } => Some(*field),
            Self::UnknownClassField { .. }
            | Self::ClassFieldOwner { .. }
            | Self::SourceClassPropertyOwner { .. }
            | Self::UnsupportedGeneratedClassOwner { .. }
            | Self::ObjectBackingPropertyOwner { .. }
            | Self::UnknownObject { .. }
            | Self::ObjectBackingRelation { .. }
            | Self::InvalidClassFieldIdentity { .. }
            | Self::GeneratedStructOwner { .. }
            | Self::TooManyStructFields { .. }
            | Self::InvalidStructFieldName { .. }
            | Self::InvalidStructFieldIdentity { .. }
            | Self::DuplicatePersistentIdentity {
                location: HirFieldIdentityLocation::Struct { .. },
            } => None,
        }
    }
}

impl fmt::Display for HirFieldIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedStructOwner { structure } => {
                write!(
                    formatter,
                    "struct {structure} has a generated nominal identity"
                )
            }
            Self::TooManyStructFields { structure } => {
                write!(
                    formatter,
                    "struct {structure} has more than u32::MAX fields"
                )
            }
            Self::InvalidStructFieldName {
                structure,
                field,
                error,
            } => write!(
                formatter,
                "struct {structure} field {field} has an invalid persistent name: {error}"
            ),
            Self::InvalidStructFieldIdentity {
                structure,
                field,
                error,
            } => write!(
                formatter,
                "struct {structure} field {field} has an invalid persistent identity: {error}"
            ),
            Self::UnknownClassField { class, field } => {
                write!(formatter, "class {class} refers to unknown field {field}")
            }
            Self::DuplicateClassField { field } => {
                write!(
                    formatter,
                    "class field {field} belongs to multiple class field lists"
                )
            }
            Self::ClassFieldOwner {
                class,
                field,
                actual_class,
            } => write!(
                formatter,
                "class {class} owns field {field}, which points to class {actual_class}"
            ),
            Self::UnknownProperty { field, property } => {
                write!(
                    formatter,
                    "class field {field} refers to unknown property {property}"
                )
            }
            Self::ExtensionPropertyField { field, property } => write!(
                formatter,
                "class field {field} refers to extension property {property}"
            ),
            Self::PropertyStorageMismatch { field, property } => write!(
                formatter,
                "class field {field} is not the physical storage of property {property}"
            ),
            Self::UnknownDelegateStorage {
                field,
                property,
                storage,
            } => write!(
                formatter,
                "class field {field} property {property} refers to unknown delegate storage {storage}"
            ),
            Self::DelegateStorageMismatch { field, property } => write!(
                formatter,
                "class field {field} and property {property} disagree with their delegate storage"
            ),
            Self::SourceClassPropertyOwner {
                class,
                field,
                property,
            } => write!(
                formatter,
                "source class {class} field {field} refers to property {property} with another owner"
            ),
            Self::UnsupportedGeneratedClassOwner { class, field } => write!(
                formatter,
                "class {class} field {field} has no export-HIR generated field role"
            ),
            Self::ObjectBackingPropertyOwner {
                class,
                field,
                property,
            } => write!(
                formatter,
                "object backing class {class} field {field} refers to non-object property {property}"
            ),
            Self::UnknownObject {
                class,
                field,
                object,
            } => write!(
                formatter,
                "object backing class {class} field {field} refers to unknown object {object}"
            ),
            Self::ObjectBackingRelation {
                class,
                field,
                object,
            } => write!(
                formatter,
                "object backing class {class} field {field} does not belong to object {object}"
            ),
            Self::InvalidClassFieldIdentity {
                class,
                field,
                error,
            } => write!(
                formatter,
                "class {class} field {field} has an invalid persistent identity: {error}"
            ),
            Self::UnownedClassField { field } => {
                write!(
                    formatter,
                    "class field {field} has no class declaration owner"
                )
            }
            Self::DuplicatePersistentIdentity { location } => {
                write!(
                    formatter,
                    "{location:?} duplicates a persistent field identity"
                )
            }
        }
    }
}

impl std::error::Error for HirFieldIdentityError {}
