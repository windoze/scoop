use super::*;

pub(super) fn struct_field_record(
    structs: &Arena<StructDecl>,
    nominal_identities: &HirNominalIdentities,
    field: StructFieldRef,
) -> Result<FieldRecord, HirFieldIdentityError> {
    let structure = field.structure();
    let structure_index = raw_index(structure);
    let field_index = field.local_index();
    let owner = nominal_identities[structure].source().ok_or(
        HirFieldIdentityError::GeneratedStructOwner {
            structure: structure_index,
        },
    )?;
    let declaration = &structs[structure].semantic_fields()[field_index as usize];
    let name = CanonicalIdentifier::new(&declaration.name).map_err(|error| {
        HirFieldIdentityError::InvalidStructFieldName {
            structure: structure_index,
            field: field_index,
            error,
        }
    })?;
    let key = FieldIdentityKey::source_declared(owner.declaration(), name).map_err(|error| {
        HirFieldIdentityError::InvalidStructFieldIdentity {
            structure: structure_index,
            field: field_index,
            error,
        }
    })?;
    CborIdentityRecord::from_key(key).map_err(|error| {
        HirFieldIdentityError::InvalidStructFieldIdentity {
            structure: structure_index,
            field: field_index,
            error,
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn class_field_record(
    class_id: ClassId,
    field_id: ClassFieldId,
    field: &ClassField,
    objects: &Arena<ObjectDecl>,
    properties: &Arena<Property>,
    delegate_storages: &Arena<DelegateStorage>,
    nominal_identities: &HirNominalIdentities,
    property_identity: &HirPropertyIdentity,
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
    let property_id = match property_identity {
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
        | PropertyRepresentation::GenericDelegated { .. }
        | PropertyRepresentation::Const { .. }
        | PropertyRepresentation::NativeStorage { .. } => {
            Err(HirFieldIdentityError::PropertyStorageMismatch {
                field,
                property: property_index,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClassStorageRole {
    Backing,
    Delegate,
}
