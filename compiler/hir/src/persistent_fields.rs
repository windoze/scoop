//! Persistent identities for source struct fields and class-backed storage.

use std::collections::{HashMap, HashSet};
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, FieldIdentityKey, GeneratedNominalKey,
    PersistentFieldId,
};

use crate::{
    AppliedStructFieldRef, ClassDecl, ClassField, ClassFieldId, ClassId, DelegateStorage,
    DelegateStorageLocation, HirNominalIdentities, HirPropertyIdentities, HirPropertyIdentity,
    ObjectDecl, Property, PropertyBacking, PropertyId, PropertyOwner, PropertyRepresentation,
    StructDecl, StructFieldRef,
};

mod builder;
mod error;
mod records;
pub use builder::HirFieldIdentityBuilder;
pub use error::{HirFieldIdentityError, HirFieldIdentityLocation};

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

#[derive(Clone, Debug)]
pub struct HirFieldIdentities {
    struct_fields: Vec<Vec<FieldRecord>>,
    class_fields: Vec<FieldRecord>,
    struct_by_identity: HashMap<PersistentFieldId, StructFieldRef>,
    class_by_identity: HashMap<PersistentFieldId, ClassFieldId>,
}

impl HirFieldIdentities {
    pub fn struct_declaration(&self, field: PersistentFieldId) -> Option<StructFieldRef> {
        self.struct_by_identity.get(&field).copied()
    }

    pub fn class_declaration(&self, field: PersistentFieldId) -> Option<ClassFieldId> {
        self.class_by_identity.get(&field).copied()
    }

    pub(crate) fn get_struct(&self, field: StructFieldRef) -> Option<&FieldRecord> {
        self.struct_fields
            .get(local_index(field.structure()))?
            .get(field.local_index() as usize)
    }
}

impl Index<StructFieldRef> for HirFieldIdentities {
    type Output = FieldRecord;

    fn index(&self, field: StructFieldRef) -> &Self::Output {
        &self.struct_fields[local_index(field.structure())][field.local_index() as usize]
    }
}

impl Index<AppliedStructFieldRef> for HirFieldIdentities {
    type Output = FieldRecord;

    fn index(&self, field: AppliedStructFieldRef) -> &Self::Output {
        &self[field.declaration()]
    }
}

impl Index<ClassFieldId> for HirFieldIdentities {
    type Output = FieldRecord;

    fn index(&self, field: ClassFieldId) -> &Self::Output {
        &self.class_fields[local_index(field)]
    }
}

fn require_unique_id(
    ids: &mut HashSet<PersistentFieldId>,
    id: PersistentFieldId,
    location: HirFieldIdentityLocation,
) -> Result<(), HirFieldIdentityError> {
    if ids.insert(id) {
        Ok(())
    } else {
        Err(HirFieldIdentityError::DuplicatePersistentIdentity { location })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[cfg(test)]
mod tests;
