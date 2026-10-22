//! Typed annotation use sites are independent of callable request contexts.

use crate::{CallableDeclaration, NominalId, Reference};
use scoop_hir as hir;
use scoop_identity::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum PropertyDeclaration {
    Property(Reference<PersistentPropertyId>),
    ExtensionProperty(Reference<PersistentExtensionPropertyId>),
}

impl From<PropertyOwner> for PropertyDeclaration {
    fn from(owner: PropertyOwner) -> Self {
        match owner {
            PropertyOwner::Property(id) => Self::Property(id.into()),
            PropertyOwner::ExtensionProperty(id) => Self::ExtensionProperty(id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationStorageRole {
    Backing,
    Delegate,
}

impl From<hir::AnnotationStorageRole> for AnnotationStorageRole {
    fn from(role: hir::AnnotationStorageRole) -> Self {
        match role {
            hir::AnnotationStorageRole::Backing => Self::Backing,
            hir::AnnotationStorageRole::Delegate => Self::Delegate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnnotationTarget {
    Nominal {
        owner: NominalId,
    },
    Variant {
        variant: Reference<PersistentEnumVariantId>,
    },
    Field {
        field: Reference<PersistentFieldId>,
    },
    VariantField {
        field: Reference<PersistentEnumVariantFieldId>,
    },
    Property {
        property: Reference<PersistentPropertyId>,
    },
    ExtensionProperty {
        property: Reference<PersistentExtensionPropertyId>,
    },
    Annotation {
        annotation: Reference<PersistentAnnotationId>,
    },
    Callable {
        callable: CallableDeclaration,
    },
    Parameter {
        callable: CallableDeclaration,
        index: u32,
    },
    CallableReceiver {
        callable: CallableDeclaration,
    },
    PropertyReceiver {
        property: PropertyDeclaration,
    },
    TypeAlias {
        alias: Reference<PersistentTypeAliasId>,
    },
    PropertyStorage {
        property: PropertyDeclaration,
        role: AnnotationStorageRole,
    },
}

impl From<hir::AnnotationCallableV1> for CallableDeclaration {
    fn from(callable: hir::AnnotationCallableV1) -> Self {
        match callable {
            hir::AnnotationCallableV1::Declaration(declaration) => declaration.into(),
            hir::AnnotationCallableV1::Local(id) => Self::Generated(id.into()),
        }
    }
}

impl From<hir::AnnotationTargetV1> for AnnotationTarget {
    fn from(target: hir::AnnotationTargetV1) -> Self {
        use hir::AnnotationTargetV1 as Target;
        match target {
            Target::Nominal(owner) => Self::Nominal {
                owner: owner.into(),
            },
            Target::Variant(id) => Self::Variant { variant: id.into() },
            Target::Field(id) => Self::Field { field: id.into() },
            Target::VariantField(id) => Self::VariantField { field: id.into() },
            Target::Property(id) => Self::Property {
                property: id.into(),
            },
            Target::ExtensionProperty(id) => Self::ExtensionProperty {
                property: id.into(),
            },
            Target::Annotation(id) => Self::Annotation {
                annotation: id.into(),
            },
            Target::Callable(callable) => Self::Callable {
                callable: callable.into(),
            },
            Target::Parameter { callable, index } => Self::Parameter {
                callable: callable.into(),
                index,
            },
            Target::CallableReceiver(callable) => Self::CallableReceiver {
                callable: callable.into(),
            },
            Target::PropertyReceiver(property) => Self::PropertyReceiver {
                property: property.into(),
            },
            Target::TypeAlias(id) => Self::TypeAlias { alias: id.into() },
            Target::PropertyStorage { property, role } => Self::PropertyStorage {
                property: property.into(),
                role: role.into(),
            },
        }
    }
}
