use scoop_identity::{PersistentEnumVariantId, PersistentObjectValueId, PersistentTypeAliasId};

use super::{
    ImportedCallableDeclarationId, ImportedPropertyDeclarationId, ImportedSourceNominalId,
};
use crate::{
    CallableInterfaceRecordV1, EnumSourceVariantV1, ImportedHirId, NominalInterfaceRecordV1,
    PropertyInterfaceRecordV1, TypeAliasExpansionV1, TypeAliasInterfaceRecordV1, WorldConeId,
};

#[derive(Clone, Copy)]
pub struct ImportedNominal<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) declaration: ImportedSourceNominalId,
    pub(in crate::semantic_world) record: &'input NominalInterfaceRecordV1,
}

impl<'input> ImportedNominal<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn declaration(self) -> ImportedSourceNominalId {
        self.declaration
    }

    pub const fn record(self) -> &'input NominalInterfaceRecordV1 {
        self.record
    }
}

#[derive(Clone, Copy)]
pub struct ImportedCallable<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) declaration: ImportedCallableDeclarationId,
    pub(in crate::semantic_world) record: &'input CallableInterfaceRecordV1,
}

impl<'input> ImportedCallable<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn declaration(self) -> ImportedCallableDeclarationId {
        self.declaration
    }

    pub const fn record(self) -> &'input CallableInterfaceRecordV1 {
        self.record
    }
}

#[derive(Clone, Copy)]
pub struct ImportedProperty<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) declaration: ImportedPropertyDeclarationId,
    pub(in crate::semantic_world) record: &'input PropertyInterfaceRecordV1,
}

impl<'input> ImportedProperty<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn declaration(self) -> ImportedPropertyDeclarationId {
        self.declaration
    }

    pub const fn record(self) -> &'input PropertyInterfaceRecordV1 {
        self.record
    }
}

#[derive(Clone, Copy)]
pub struct ImportedTypeAlias<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) identity: ImportedHirId<PersistentTypeAliasId>,
    pub(in crate::semantic_world) record: &'input TypeAliasInterfaceRecordV1,
    pub(in crate::semantic_world) expansion: &'input TypeAliasExpansionV1,
}

impl<'input> ImportedTypeAlias<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn identity(self) -> ImportedHirId<PersistentTypeAliasId> {
        self.identity
    }

    pub const fn record(self) -> &'input TypeAliasInterfaceRecordV1 {
        self.record
    }

    pub const fn expansion(self) -> &'input TypeAliasExpansionV1 {
        self.expansion
    }
}

#[derive(Clone, Copy)]
pub struct ImportedObjectValue<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) identity: ImportedHirId<PersistentObjectValueId>,
    pub(in crate::semantic_world) owner: ImportedSourceNominalId,
    pub(in crate::semantic_world) owner_record: &'input NominalInterfaceRecordV1,
}

impl<'input> ImportedObjectValue<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn identity(self) -> ImportedHirId<PersistentObjectValueId> {
        self.identity
    }

    pub const fn owner(self) -> ImportedSourceNominalId {
        self.owner
    }

    pub const fn owner_record(self) -> &'input NominalInterfaceRecordV1 {
        self.owner_record
    }
}

#[derive(Clone, Copy)]
pub struct ImportedEnumVariant<'input> {
    pub(in crate::semantic_world) provider: WorldConeId,
    pub(in crate::semantic_world) identity: ImportedHirId<PersistentEnumVariantId>,
    pub(in crate::semantic_world) owner: ImportedSourceNominalId,
    pub(in crate::semantic_world) owner_record: &'input NominalInterfaceRecordV1,
    pub(in crate::semantic_world) record: &'input EnumSourceVariantV1,
}

impl<'input> ImportedEnumVariant<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn identity(self) -> ImportedHirId<PersistentEnumVariantId> {
        self.identity
    }

    pub const fn owner(self) -> ImportedSourceNominalId {
        self.owner
    }

    pub const fn owner_record(self) -> &'input NominalInterfaceRecordV1 {
        self.owner_record
    }

    pub const fn record(self) -> &'input EnumSourceVariantV1 {
        self.record
    }
}
