use crate::Reference;
use scoop_identity::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum MethodId {
    Function(Reference<PersistentFunctionId>),
    GenericFunction(Reference<PersistentGenericFunctionId>),
}

impl From<&scoop_hir::HirSourceFunctionIdentity> for MethodId {
    fn from(identity: &scoop_hir::HirSourceFunctionIdentity) -> Self {
        match identity {
            scoop_hir::HirSourceFunctionIdentity::Plain(record) => {
                Self::Function(record.id().into())
            }
            scoop_hir::HirSourceFunctionIdentity::Generic(record) => {
                Self::GenericFunction(record.id().into())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum CallableDeclaration {
    Function(Reference<PersistentFunctionId>),
    GenericFunction(Reference<PersistentGenericFunctionId>),
    Constructor(Reference<PersistentConstructorId>),
    PropertyAccessor(Reference<PersistentPropertyAccessorId>),
    VariantConstructor(Reference<PersistentEnumVariantId>),
    Generated(Reference<PersistentGeneratedCallableId>),
}

impl From<CallableTemplateOrigin> for CallableDeclaration {
    fn from(value: CallableTemplateOrigin) -> Self {
        match value {
            CallableTemplateOrigin::Function(id) => Self::Function(id.into()),
            CallableTemplateOrigin::GenericFunction(id) => Self::GenericFunction(id.into()),
            CallableTemplateOrigin::Constructor(id) => Self::Constructor(id.into()),
            CallableTemplateOrigin::Accessor(id) => Self::PropertyAccessor(id.into()),
            CallableTemplateOrigin::VariantConstructor(id) => Self::VariantConstructor(id.into()),
        }
    }
}

impl From<scoop_hir::DefaultCallableDeclarationV1> for CallableDeclaration {
    fn from(value: scoop_hir::DefaultCallableDeclarationV1) -> Self {
        use scoop_hir::DefaultCallableDeclarationV1 as Declaration;
        match value {
            Declaration::Function(id) => Self::Function(id.into()),
            Declaration::GenericFunction(id) => Self::GenericFunction(id.into()),
            Declaration::PropertyAccessor(id) => Self::PropertyAccessor(id.into()),
            Declaration::Generated(id) => Self::Generated(id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum NominalId {
    Concrete(Reference<PersistentTypeId>),
    Generic(Reference<PersistentGenericTypeId>),
}

impl From<scoop_hir::SourceNominalId> for NominalId {
    fn from(value: scoop_hir::SourceNominalId) -> Self {
        match value {
            scoop_hir::SourceNominalId::Concrete(id) => Self::Concrete(id.into()),
            scoop_hir::SourceNominalId::GenericTemplate(id) => Self::Generic(id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum FieldId {
    Field(Reference<PersistentFieldId>),
    VariantField(Reference<PersistentEnumVariantFieldId>),
}

impl From<scoop_hir::StaticFieldIdentity> for FieldId {
    fn from(value: scoop_hir::StaticFieldIdentity) -> Self {
        match value {
            scoop_hir::StaticFieldIdentity::Field(id) => Self::Field(id.into()),
            scoop_hir::StaticFieldIdentity::VariantField(id) => Self::VariantField(id.into()),
        }
    }
}
