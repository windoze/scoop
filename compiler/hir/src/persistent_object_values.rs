//! Persistent identities aligned with source object singleton values.

use std::fmt;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CborIdentityRecord, PersistentObjectValueId, SourceDeclarationIdentityError,
    SourceDeclarationKey,
};

use crate::{
    HirNominalIdentities, ObjectDecl, ObjectId, ObjectType, SingletonValue, SingletonValueId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectValueIdentity {
    declaration: ObjectId,
    record: CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey>,
}

impl HirObjectValueIdentity {
    pub const fn declaration(&self) -> ObjectId {
        self.declaration
    }

    pub const fn record(
        &self,
    ) -> &CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey> {
        &self.record
    }

    pub const fn id(&self) -> PersistentObjectValueId {
        self.record.id()
    }
}

/// Total object declaration/type/value relation indexed by singleton value.
#[derive(Clone, Debug)]
pub struct HirObjectValueIdentities {
    values: Vec<HirObjectValueIdentity>,
}

impl HirObjectValueIdentities {
    pub fn from_declarations(
        objects: &Arena<ObjectDecl>,
        object_types: &Arena<ObjectType>,
        singleton_values: &Arena<SingletonValue>,
        nominal_identities: &HirNominalIdentities,
    ) -> Result<Self, HirObjectValueIdentityError> {
        require_count(
            ObjectRelationTable::SingletonValue,
            objects.len(),
            singleton_values.len(),
        )?;
        require_count(
            ObjectRelationTable::ObjectType,
            objects.len(),
            object_types.len(),
        )?;

        let mut seen_values = vec![false; singleton_values.len()];
        let mut seen_types = vec![false; object_types.len()];
        for (object_id, object) in objects.iter() {
            let object_index = raw_index(object_id);
            let value_index = local_index(object.singleton_value);
            if value_index >= singleton_values.len() {
                return Err(HirObjectValueIdentityError::UnknownSingletonValue {
                    object: object_index,
                    value: raw_index(object.singleton_value),
                });
            }
            let value = &singleton_values[object.singleton_value];
            if seen_values[value_index] {
                return Err(HirObjectValueIdentityError::DuplicateSingletonValue {
                    value: raw_index(object.singleton_value),
                });
            }
            seen_values[value_index] = true;
            if value.declaration != object_id {
                return Err(HirObjectValueIdentityError::ValueBackReference {
                    object: object_index,
                    value: raw_index(object.singleton_value),
                    actual_object: raw_index(value.declaration),
                });
            }

            let object_type_index = local_index(object.object_type);
            if object_type_index >= object_types.len() {
                return Err(HirObjectValueIdentityError::UnknownObjectType {
                    object: object_index,
                    object_type: raw_index(object.object_type),
                });
            }
            let object_type = &object_types[object.object_type];
            if seen_types[object_type_index] {
                return Err(HirObjectValueIdentityError::DuplicateObjectType {
                    object_type: raw_index(object.object_type),
                });
            }
            seen_types[object_type_index] = true;
            if object_type.declaration != object_id {
                return Err(HirObjectValueIdentityError::ObjectTypeBackReference {
                    object: object_index,
                    object_type: raw_index(object.object_type),
                    actual_object: raw_index(object_type.declaration),
                });
            }
            if value.object_type != object.object_type {
                return Err(HirObjectValueIdentityError::ValueObjectType {
                    object: object_index,
                    value: raw_index(object.singleton_value),
                    expected: raw_index(object.object_type),
                    actual: raw_index(value.object_type),
                });
            }
        }
        require_coverage(ObjectRelationTable::SingletonValue, &seen_values)?;
        require_coverage(ObjectRelationTable::ObjectType, &seen_types)?;

        let mut identities = Vec::with_capacity(singleton_values.len());
        for (value_id, value) in singleton_values.iter() {
            let object_index = raw_index(value.declaration);
            let identity = nominal_identities[value.declaration].source().ok_or(
                HirObjectValueIdentityError::GeneratedObjectIdentity {
                    object: object_index,
                },
            )?;
            let record =
                CborIdentityRecord::from_key(identity.declaration().clone()).map_err(|error| {
                    HirObjectValueIdentityError::InvalidIdentity {
                        object: object_index,
                        value: raw_index(value_id),
                        error,
                    }
                })?;
            identities.push(HirObjectValueIdentity {
                declaration: value.declaration,
                record,
            });
        }
        Ok(Self { values: identities })
    }
}

impl Index<SingletonValueId> for HirObjectValueIdentities {
    type Output = HirObjectValueIdentity;

    fn index(&self, id: SingletonValueId) -> &Self::Output {
        &self.values[local_index(id)]
    }
}

fn require_count(
    table: ObjectRelationTable,
    expected: usize,
    actual: usize,
) -> Result<(), HirObjectValueIdentityError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirObjectValueIdentityError::Count {
            table,
            expected,
            actual,
        })
    }
}

fn require_coverage(
    table: ObjectRelationTable,
    seen: &[bool],
) -> Result<(), HirObjectValueIdentityError> {
    if let Some(index) = seen.iter().position(|seen| !seen) {
        Err(HirObjectValueIdentityError::Unowned {
            table,
            index: index as u32,
        })
    } else {
        Ok(())
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectRelationTable {
    SingletonValue,
    ObjectType,
}

impl fmt::Display for ObjectRelationTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SingletonValue => "singleton-value",
            Self::ObjectType => "object-type",
        })
    }
}

#[derive(Debug)]
pub enum HirObjectValueIdentityError {
    Count {
        table: ObjectRelationTable,
        expected: usize,
        actual: usize,
    },
    UnknownSingletonValue {
        object: u32,
        value: u32,
    },
    DuplicateSingletonValue {
        value: u32,
    },
    ValueBackReference {
        object: u32,
        value: u32,
        actual_object: u32,
    },
    UnknownObjectType {
        object: u32,
        object_type: u32,
    },
    DuplicateObjectType {
        object_type: u32,
    },
    ObjectTypeBackReference {
        object: u32,
        object_type: u32,
        actual_object: u32,
    },
    ValueObjectType {
        object: u32,
        value: u32,
        expected: u32,
        actual: u32,
    },
    Unowned {
        table: ObjectRelationTable,
        index: u32,
    },
    GeneratedObjectIdentity {
        object: u32,
    },
    InvalidIdentity {
        object: u32,
        value: u32,
        error: SourceDeclarationIdentityError,
    },
}

impl HirObjectValueIdentityError {
    pub const fn object(&self) -> Option<u32> {
        match self {
            Self::UnknownSingletonValue { object, .. }
            | Self::ValueBackReference { object, .. }
            | Self::UnknownObjectType { object, .. }
            | Self::ObjectTypeBackReference { object, .. }
            | Self::ValueObjectType { object, .. }
            | Self::GeneratedObjectIdentity { object }
            | Self::InvalidIdentity { object, .. } => Some(*object),
            Self::Count { .. }
            | Self::DuplicateSingletonValue { .. }
            | Self::DuplicateObjectType { .. }
            | Self::Unowned { .. } => None,
        }
    }
}

impl fmt::Display for HirObjectValueIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count {
                table,
                expected,
                actual,
            } => write!(
                formatter,
                "object relation has {actual} {table} entries, expected {expected}"
            ),
            Self::UnknownSingletonValue { object, value } => {
                write!(
                    formatter,
                    "object {object} refers to unknown singleton value {value}"
                )
            }
            Self::DuplicateSingletonValue { value } => {
                write!(
                    formatter,
                    "singleton value {value} belongs to multiple objects"
                )
            }
            Self::ValueBackReference {
                object,
                value,
                actual_object,
            } => write!(
                formatter,
                "object {object} refers to singleton value {value}, which points to object {actual_object}"
            ),
            Self::UnknownObjectType {
                object,
                object_type,
            } => write!(
                formatter,
                "object {object} refers to unknown object type {object_type}"
            ),
            Self::DuplicateObjectType { object_type } => {
                write!(
                    formatter,
                    "object type {object_type} belongs to multiple objects"
                )
            }
            Self::ObjectTypeBackReference {
                object,
                object_type,
                actual_object,
            } => write!(
                formatter,
                "object {object} refers to object type {object_type}, which points to object {actual_object}"
            ),
            Self::ValueObjectType {
                object,
                value,
                expected,
                actual,
            } => write!(
                formatter,
                "object {object} singleton value {value} refers to object type {actual}, expected {expected}"
            ),
            Self::Unowned { table, index } => {
                write!(formatter, "{table} entry {index} has no object declaration")
            }
            Self::GeneratedObjectIdentity { object } => write!(
                formatter,
                "source object {object} has a generated nominal identity"
            ),
            Self::InvalidIdentity {
                object,
                value,
                error,
            } => write!(
                formatter,
                "object {object} singleton value {value} has an invalid persistent identity: {error}"
            ),
        }
    }
}

impl std::error::Error for HirObjectValueIdentityError {}

#[cfg(test)]
mod tests;
