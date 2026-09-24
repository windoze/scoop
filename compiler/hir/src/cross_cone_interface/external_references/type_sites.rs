//! Actual expression, signature and declaration types in shared HIR metadata.

use crate::concrete::ExecutableExpressionPosition;
use scoop_identity::{
    CallableMaterialization, ConcreteExpressionOrigin, PersistentEnumVariantFieldId,
    PersistentExactTypeId, PersistentFieldId, PersistentInitializationUnitId,
    PersistentLocalValueId, PropertyOwner,
};

mod decode;
mod dependencies;
mod errors;
mod expression;
mod nominals;
mod position;
mod relations;
mod role;
mod table;
#[cfg(test)]
mod tests;
mod wire;

pub use decode::{DecodedHirDependencyTypeSiteV1, HirDependencyTypeSiteResolver};
pub use errors::{HirDependencyTypeSiteBuildError, HirDependencyTypeSiteResolutionError};
pub use expression::HirExpressionTypeSiteV1;
pub use nominals::{HirTypeSiteExactError, collect_type_site_nominals};
pub use position::{HirCallableTypePositionV1, HirDependencyTypePositionV1};
pub use relations::HirDependencyTypeRelationError;
pub use role::HirExpressionTypeRoleV1;
pub use table::{CanonicalHirDependencyTypeSitesV1, DecodedCanonicalHirDependencyTypeSitesV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDependencyTypeSiteV1 {
    Expression(Box<HirExpressionTypeSiteV1>),
    CallableSignature {
        root: CallableMaterialization,
        position: HirCallableTypePositionV1,
        exact: PersistentExactTypeId,
    },
    LocalValue {
        local: PersistentLocalValueId,
        exact: PersistentExactTypeId,
    },
    BackingStorage {
        property: PropertyOwner,
        exact: PersistentExactTypeId,
    },
    DelegateStorage {
        property: PropertyOwner,
        exact: PersistentExactTypeId,
    },
    FieldStorage {
        field: PersistentFieldId,
        exact: PersistentExactTypeId,
    },
    EnumVariantFieldStorage {
        field: PersistentEnumVariantFieldId,
        exact: PersistentExactTypeId,
    },
    ConstructorInitializerResult {
        constructor: CallableMaterialization,
        exact: PersistentExactTypeId,
    },
    InitializationCycleMessage {
        unit: PersistentInitializationUnitId,
        exact: PersistentExactTypeId,
    },
}

impl HirDependencyTypeSiteV1 {
    pub fn new(
        position: ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        role: HirExpressionTypeRoleV1,
        exact: PersistentExactTypeId,
    ) -> Self {
        Self::Expression(Box::new(HirExpressionTypeSiteV1::new(
            position, origin, role, exact,
        )))
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        match self {
            Self::Expression(site) => site.exact(),
            Self::CallableSignature { exact, .. }
            | Self::LocalValue { exact, .. }
            | Self::BackingStorage { exact, .. }
            | Self::DelegateStorage { exact, .. }
            | Self::FieldStorage { exact, .. }
            | Self::EnumVariantFieldStorage { exact, .. }
            | Self::ConstructorInitializerResult { exact, .. }
            | Self::InitializationCycleMessage { exact, .. } => *exact,
        }
    }

    pub const fn as_expression(&self) -> Option<&HirExpressionTypeSiteV1> {
        match self {
            Self::Expression(site) => Some(site),
            Self::CallableSignature { .. }
            | Self::LocalValue { .. }
            | Self::BackingStorage { .. }
            | Self::DelegateStorage { .. }
            | Self::FieldStorage { .. }
            | Self::EnumVariantFieldStorage { .. }
            | Self::ConstructorInitializerResult { .. }
            | Self::InitializationCycleMessage { .. } => None,
        }
    }

    pub const fn position(&self) -> HirDependencyTypePositionV1 {
        match self {
            Self::Expression(site) => {
                HirDependencyTypePositionV1::Expression(site.position(), site.role())
            }
            Self::CallableSignature { root, position, .. } => {
                HirDependencyTypePositionV1::CallableSignature(*root, *position)
            }
            Self::LocalValue { local, .. } => HirDependencyTypePositionV1::LocalValue(*local),
            Self::BackingStorage { property, .. } => {
                HirDependencyTypePositionV1::BackingStorage(*property)
            }
            Self::DelegateStorage { property, .. } => {
                HirDependencyTypePositionV1::DelegateStorage(*property)
            }
            Self::FieldStorage { field, .. } => HirDependencyTypePositionV1::FieldStorage(*field),
            Self::EnumVariantFieldStorage { field, .. } => {
                HirDependencyTypePositionV1::EnumVariantFieldStorage(*field)
            }
            Self::ConstructorInitializerResult { constructor, .. } => {
                HirDependencyTypePositionV1::ConstructorInitializerResult(*constructor)
            }
            Self::InitializationCycleMessage { unit, .. } => {
                HirDependencyTypePositionV1::InitializationCycleMessage(*unit)
            }
        }
    }
}
