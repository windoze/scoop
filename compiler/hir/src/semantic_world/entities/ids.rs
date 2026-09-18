use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, PersistentConstructorId, PersistentEnumVariantId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, PropertyOwner,
};

use crate::{ImportedHirId, SourceNominalId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportedSourceNominalId {
    Concrete(ImportedHirId<PersistentTypeId>),
    GenericTemplate(ImportedHirId<PersistentGenericTypeId>),
}

impl ImportedSourceNominalId {
    pub const fn persistent(self) -> SourceNominalId {
        match self {
            Self::Concrete(id) => SourceNominalId::Concrete(id.persistent()),
            Self::GenericTemplate(id) => SourceNominalId::GenericTemplate(id.persistent()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportedCallableDeclarationId {
    Function(ImportedHirId<PersistentFunctionId>),
    GenericFunction(ImportedHirId<PersistentGenericFunctionId>),
    Constructor(ImportedHirId<PersistentConstructorId>),
    Accessor(ImportedHirId<PersistentPropertyAccessorId>),
    VariantConstructor(ImportedHirId<PersistentEnumVariantId>),
}

impl ImportedCallableDeclarationId {
    pub const fn persistent(self) -> CallableTemplateOrigin {
        match self {
            Self::Function(id) => CallableTemplateOrigin::Function(id.persistent()),
            Self::GenericFunction(id) => CallableTemplateOrigin::GenericFunction(id.persistent()),
            Self::Constructor(id) => CallableTemplateOrigin::Constructor(id.persistent()),
            Self::Accessor(id) => CallableTemplateOrigin::Accessor(id.persistent()),
            Self::VariantConstructor(id) => {
                CallableTemplateOrigin::VariantConstructor(id.persistent())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportedPropertyDeclarationId {
    Property(ImportedHirId<PersistentPropertyId>),
    ExtensionProperty(ImportedHirId<PersistentExtensionPropertyId>),
}

impl ImportedPropertyDeclarationId {
    pub const fn persistent(self) -> PropertyOwner {
        match self {
            Self::Property(id) => PropertyOwner::Property(id.persistent()),
            Self::ExtensionProperty(id) => PropertyOwner::ExtensionProperty(id.persistent()),
        }
    }
}

/// Kind-preserving imported target. The session-local identity can never be
/// confused with a current-Cone arena id or another persistent-id family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportedTarget {
    Type(ImportedHirId<PersistentTypeId>),
    GenericType(ImportedHirId<PersistentGenericTypeId>),
    ObjectValue(ImportedHirId<PersistentObjectValueId>),
    Function(ImportedHirId<PersistentFunctionId>),
    GenericFunction(ImportedHirId<PersistentGenericFunctionId>),
    Property(ImportedHirId<PersistentPropertyId>),
    ExtensionProperty(ImportedHirId<PersistentExtensionPropertyId>),
    TypeAlias(ImportedHirId<PersistentTypeAliasId>),
    EnumVariant(ImportedHirId<PersistentEnumVariantId>),
}

impl ImportedTarget {
    pub const fn persistent(self) -> BindableEntity {
        match self {
            Self::Type(id) => BindableEntity::Type(id.persistent()),
            Self::GenericType(id) => BindableEntity::GenericType(id.persistent()),
            Self::ObjectValue(id) => BindableEntity::ObjectValue(id.persistent()),
            Self::Function(id) => BindableEntity::Function(id.persistent()),
            Self::GenericFunction(id) => BindableEntity::GenericFunction(id.persistent()),
            Self::Property(id) => BindableEntity::Property(id.persistent()),
            Self::ExtensionProperty(id) => BindableEntity::ExtensionProperty(id.persistent()),
            Self::TypeAlias(id) => BindableEntity::TypeAlias(id.persistent()),
            Self::EnumVariant(id) => BindableEntity::EnumVariant(id.persistent()),
        }
    }

    pub const fn source_nominal(self) -> Option<SourceNominalId> {
        match self {
            Self::Type(id) => Some(SourceNominalId::Concrete(id.persistent())),
            Self::GenericType(id) => Some(SourceNominalId::GenericTemplate(id.persistent())),
            Self::ObjectValue(_)
            | Self::Function(_)
            | Self::GenericFunction(_)
            | Self::Property(_)
            | Self::ExtensionProperty(_)
            | Self::TypeAlias(_)
            | Self::EnumVariant(_) => None,
        }
    }
}
