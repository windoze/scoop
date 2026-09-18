//! Persistent identities for source struct fields and class-backed storage.

use std::collections::HashSet;
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

mod error;
pub use error::{HirFieldIdentityError, HirFieldIdentityLocation};

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

#[derive(Clone, Debug)]
pub struct HirFieldIdentities {
    struct_fields: Vec<Vec<FieldRecord>>,
    class_fields: Vec<FieldRecord>,
}

impl HirFieldIdentities {
    #[allow(clippy::too_many_arguments)]
    pub fn from_declarations(
        structs: &Arena<StructDecl>,
        classes: &Arena<ClassDecl>,
        objects: &Arena<ObjectDecl>,
        class_fields: &Arena<ClassField>,
        properties: &Arena<Property>,
        delegate_storages: &Arena<DelegateStorage>,
        nominal_identities: &HirNominalIdentities,
        property_identities: &HirPropertyIdentities,
    ) -> Result<Self, HirFieldIdentityError> {
        let mut persistent_ids = HashSet::new();
        let mut struct_rows = Vec::with_capacity(structs.len());
        for (struct_id, structure) in structs.iter() {
            let structure_index = raw_index(struct_id);
            let owner = nominal_identities[struct_id].source().ok_or(
                HirFieldIdentityError::GeneratedStructOwner {
                    structure: structure_index,
                },
            )?;
            let mut records = Vec::with_capacity(structure.semantic_fields().len());
            for (field_index, field) in structure.semantic_fields().iter().enumerate() {
                let field_index = u32::try_from(field_index).map_err(|_| {
                    HirFieldIdentityError::TooManyStructFields {
                        structure: structure_index,
                    }
                })?;
                let name = CanonicalIdentifier::new(&field.name).map_err(|error| {
                    HirFieldIdentityError::InvalidStructFieldName {
                        structure: structure_index,
                        field: field_index,
                        error,
                    }
                })?;
                let key = FieldIdentityKey::source_declared(owner.declaration(), name).map_err(
                    |error| HirFieldIdentityError::InvalidStructFieldIdentity {
                        structure: structure_index,
                        field: field_index,
                        error,
                    },
                )?;
                let record = CborIdentityRecord::from_key(key).map_err(|error| {
                    HirFieldIdentityError::InvalidStructFieldIdentity {
                        structure: structure_index,
                        field: field_index,
                        error,
                    }
                })?;
                require_unique_id(
                    &mut persistent_ids,
                    record.id(),
                    HirFieldIdentityLocation::Struct {
                        structure: structure_index,
                        field: field_index,
                    },
                )?;
                records.push(record);
            }
            struct_rows.push(records);
        }

        let mut seen_class_fields = vec![false; class_fields.len()];
        let mut class_records = vec![None; class_fields.len()];
        for (class_id, class) in classes.iter() {
            for field_id in &class.fields {
                let field_index = local_index(*field_id);
                if field_index >= class_fields.len() {
                    return Err(HirFieldIdentityError::UnknownClassField {
                        class: raw_index(class_id),
                        field: raw_index(*field_id),
                    });
                }
                if seen_class_fields[field_index] {
                    return Err(HirFieldIdentityError::DuplicateClassField {
                        field: raw_index(*field_id),
                    });
                }
                seen_class_fields[field_index] = true;
                let field = &class_fields[*field_id];
                let record = class_field_record(
                    class_id,
                    *field_id,
                    field,
                    objects,
                    properties,
                    delegate_storages,
                    nominal_identities,
                    property_identities,
                )?;
                require_unique_id(
                    &mut persistent_ids,
                    record.id(),
                    HirFieldIdentityLocation::ClassField {
                        field: raw_index(*field_id),
                    },
                )?;
                class_records[field_index] = Some(record);
            }
        }
        if let Some(field) = seen_class_fields.iter().position(|seen| !seen) {
            return Err(HirFieldIdentityError::UnownedClassField {
                field: field as u32,
            });
        }
        let class_fields = class_records
            .into_iter()
            .enumerate()
            .map(|(field, record)| {
                record.ok_or(HirFieldIdentityError::UnownedClassField {
                    field: field as u32,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            struct_fields: struct_rows,
            class_fields,
        })
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

#[allow(clippy::too_many_arguments)]
fn class_field_record(
    class_id: ClassId,
    field_id: ClassFieldId,
    field: &ClassField,
    objects: &Arena<ObjectDecl>,
    properties: &Arena<Property>,
    delegate_storages: &Arena<DelegateStorage>,
    nominal_identities: &HirNominalIdentities,
    property_identities: &HirPropertyIdentities,
) -> Result<FieldRecord, HirFieldIdentityError> {
    let class = raw_index(class_id);
    let field_index = raw_index(field_id);
    if field.owner != class_id {
        return Err(HirFieldIdentityError::ClassFieldOwner {
            class,
            field: field_index,
            actual_class: raw_index(field.owner),
        });
    }
    if local_index(field.property) >= properties.len() {
        return Err(HirFieldIdentityError::UnknownProperty {
            field: field_index,
            property: raw_index(field.property),
        });
    }
    let property = &properties[field.property];
    let property_id = match &property_identities[field.property] {
        HirPropertyIdentity::Ordinary(record) => record.id(),
        HirPropertyIdentity::Extension(_) => {
            return Err(HirFieldIdentityError::ExtensionPropertyField {
                field: field_index,
                property: raw_index(field.property),
            });
        }
    };
    let role = storage_role(field_id, field.property, property, delegate_storages)?;
    let key = match &nominal_identities[class_id] {
        crate::HirNominalIdentity::Source(owner) => {
            if property.owner != PropertyOwner::Class(class_id) {
                return Err(HirFieldIdentityError::SourceClassPropertyOwner {
                    class,
                    field: field_index,
                    property: raw_index(field.property),
                });
            }
            match role {
                ClassStorageRole::Backing => {
                    FieldIdentityKey::source_property_backing(owner.declaration(), property_id)
                }
                ClassStorageRole::Delegate => {
                    FieldIdentityKey::source_property_delegate(owner.declaration(), property_id)
                }
            }
        }
        crate::HirNominalIdentity::Generated(owner) => {
            let GeneratedNominalKey::ObjectBackingClass { object } = owner.key() else {
                return Err(HirFieldIdentityError::UnsupportedGeneratedClassOwner {
                    class,
                    field: field_index,
                });
            };
            let PropertyOwner::Object(object_id) = property.owner else {
                return Err(HirFieldIdentityError::ObjectBackingPropertyOwner {
                    class,
                    field: field_index,
                    property: raw_index(field.property),
                });
            };
            if local_index(object_id) >= objects.len() {
                return Err(HirFieldIdentityError::UnknownObject {
                    class,
                    field: field_index,
                    object: raw_index(object_id),
                });
            }
            if objects[object_id].backing_class != class_id
                || nominal_identities[object_id].concrete_type_id() != Some(*object)
            {
                return Err(HirFieldIdentityError::ObjectBackingRelation {
                    class,
                    field: field_index,
                    object: raw_index(object_id),
                });
            }
            FieldIdentityKey::object_backing_property(owner.key(), property_id)
        }
    }
    .map_err(|error| HirFieldIdentityError::InvalidClassFieldIdentity {
        class,
        field: field_index,
        error,
    })?;
    CborIdentityRecord::from_key(key).map_err(|error| {
        HirFieldIdentityError::InvalidClassFieldIdentity {
            class,
            field: field_index,
            error,
        }
    })
}

fn storage_role(
    field_id: ClassFieldId,
    property_id: PropertyId,
    property: &Property,
    delegate_storages: &Arena<DelegateStorage>,
) -> Result<ClassStorageRole, HirFieldIdentityError> {
    let field = raw_index(field_id);
    let property_index = raw_index(property_id);
    match &property.representation {
        PropertyRepresentation::Stored(stored) => match stored.backing {
            PropertyBacking::ClassField { field: backing, .. } if backing == field_id => {
                Ok(ClassStorageRole::Backing)
            }
            _ => Err(HirFieldIdentityError::PropertyStorageMismatch {
                field,
                property: property_index,
            }),
        },
        PropertyRepresentation::Delegated { storage } => {
            if local_index(*storage) >= delegate_storages.len() {
                return Err(HirFieldIdentityError::UnknownDelegateStorage {
                    field,
                    property: property_index,
                    storage: raw_index(*storage),
                });
            }
            let storage = &delegate_storages[*storage];
            if storage.property != property_id
                || storage.location != DelegateStorageLocation::ClassField(field_id)
            {
                return Err(HirFieldIdentityError::DelegateStorageMismatch {
                    field,
                    property: property_index,
                });
            }
            Ok(ClassStorageRole::Delegate)
        }
        PropertyRepresentation::AccessorOnly
        | PropertyRepresentation::Const { .. }
        | PropertyRepresentation::NativeStorage { .. } => {
            Err(HirFieldIdentityError::PropertyStorageMismatch {
                field,
                property: property_index,
            })
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClassStorageRole {
    Backing,
    Delegate,
}
fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[cfg(test)]
mod tests;
