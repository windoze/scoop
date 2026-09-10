//! Persistent identities aligned with the export HIR accessor arenas.

use std::fmt;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    AccessorRole, CborIdentityRecord, PersistentPropertyAccessorId, PropertyAccessorKey,
    PropertyOwner as PersistentPropertyOwner,
};

use crate::{
    HirPropertyIdentities, Property, PropertyGetter, PropertyGetterId, PropertyId, PropertySetter,
    PropertySetterId,
};

/// One accessor identity together with its mandatory logical property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPropertyAccessorIdentity {
    property: PropertyId,
    record: CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>,
}

impl HirPropertyAccessorIdentity {
    pub fn getter(
        property: PropertyId,
        owner: PersistentPropertyOwner,
    ) -> Result<Self, HirPropertyAccessorIdentityError> {
        Self::new(property, owner, AccessorRole::Getter)
    }

    pub fn setter(
        property: PropertyId,
        owner: PersistentPropertyOwner,
    ) -> Result<Self, HirPropertyAccessorIdentityError> {
        Self::new(property, owner, AccessorRole::Setter)
    }

    pub const fn property(&self) -> PropertyId {
        self.property
    }

    pub const fn record(
        &self,
    ) -> &CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey> {
        &self.record
    }

    pub const fn id(&self) -> PersistentPropertyAccessorId {
        self.record.id()
    }

    fn new(
        property: PropertyId,
        owner: PersistentPropertyOwner,
        role: AccessorRole,
    ) -> Result<Self, HirPropertyAccessorIdentityError> {
        Ok(Self {
            property,
            record: CborIdentityRecord::from_key(PropertyAccessorKey::new(owner, role))
                .map_err(HirPropertyAccessorIdentityError::Hash)?,
        })
    }
}

#[derive(Debug)]
pub enum HirPropertyAccessorIdentityError {
    Hash(scoop_wire::HashError),
}

impl fmt::Display for HirPropertyAccessorIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HirPropertyAccessorIdentityError {}

/// Total persistent identity relation for both export HIR accessor arenas.
#[derive(Clone, Debug)]
pub struct HirPropertyAccessorIdentities {
    getters: Vec<HirPropertyAccessorIdentity>,
    setters: Vec<HirPropertyAccessorIdentity>,
}

impl HirPropertyAccessorIdentities {
    pub fn checked(
        properties: &Arena<Property>,
        property_identities: &HirPropertyIdentities,
        getters: &Arena<PropertyGetter>,
        getter_identities: Vec<HirPropertyAccessorIdentity>,
        setters: &Arena<PropertySetter>,
        setter_identities: Vec<HirPropertyAccessorIdentity>,
    ) -> Result<Self, HirPropertyAccessorIdentityTableError> {
        Self::check_length(
            AccessorTable::Getter,
            getters.len(),
            getter_identities.len(),
        )?;
        Self::check_length(
            AccessorTable::Setter,
            setters.len(),
            setter_identities.len(),
        )?;

        let mut seen_getters = vec![false; getters.len()];
        let mut seen_setters = vec![false; setters.len()];
        for (property_id, property) in properties.iter() {
            let owner = property_identities[property_id].property_owner();
            Self::check_entry(
                property_id,
                raw_index(property.capability.getter()),
                owner,
                AccessorTable::Getter,
                &getter_identities,
                &mut seen_getters,
            )?;
            if let Some(setter) = property.capability.setter() {
                Self::check_entry(
                    property_id,
                    raw_index(setter),
                    owner,
                    AccessorTable::Setter,
                    &setter_identities,
                    &mut seen_setters,
                )?;
            }
        }
        Self::check_coverage(AccessorTable::Getter, &seen_getters)?;
        Self::check_coverage(AccessorTable::Setter, &seen_setters)?;

        Ok(Self {
            getters: getter_identities,
            setters: setter_identities,
        })
    }

    fn check_length(
        table: AccessorTable,
        expected: usize,
        actual: usize,
    ) -> Result<(), HirPropertyAccessorIdentityTableError> {
        if expected == actual {
            Ok(())
        } else {
            Err(HirPropertyAccessorIdentityTableError::Length {
                table,
                expected,
                actual,
            })
        }
    }

    fn check_entry(
        property: PropertyId,
        accessor: u32,
        owner: PersistentPropertyOwner,
        table: AccessorTable,
        identities: &[HirPropertyAccessorIdentity],
        seen: &mut [bool],
    ) -> Result<(), HirPropertyAccessorIdentityTableError> {
        let index = accessor as usize;
        let Some(identity) = identities.get(index) else {
            return Err(HirPropertyAccessorIdentityTableError::UnknownAccessor {
                table,
                property: raw_index(property),
                accessor,
            });
        };
        if seen[index] {
            return Err(HirPropertyAccessorIdentityTableError::DuplicateAccessor {
                table,
                accessor,
            });
        }
        seen[index] = true;
        if identity.property != property {
            return Err(
                HirPropertyAccessorIdentityTableError::PropertyBackReference {
                    table,
                    property: raw_index(property),
                    accessor,
                    actual_property: raw_index(identity.property),
                },
            );
        }
        if identity.record.key().role() != table.role() {
            return Err(HirPropertyAccessorIdentityTableError::Role { table, accessor });
        }
        if identity.record.key().owner() != owner {
            return Err(HirPropertyAccessorIdentityTableError::Owner { table, accessor });
        }
        Ok(())
    }

    fn check_coverage(
        table: AccessorTable,
        seen: &[bool],
    ) -> Result<(), HirPropertyAccessorIdentityTableError> {
        if let Some(accessor) = seen.iter().position(|seen| !seen) {
            Err(HirPropertyAccessorIdentityTableError::UnownedAccessor {
                table,
                accessor: accessor as u32,
            })
        } else {
            Ok(())
        }
    }
}

impl Index<PropertyGetterId> for HirPropertyAccessorIdentities {
    type Output = HirPropertyAccessorIdentity;

    fn index(&self, id: PropertyGetterId) -> &Self::Output {
        &self.getters[local_index(id)]
    }
}

impl Index<PropertySetterId> for HirPropertyAccessorIdentities {
    type Output = HirPropertyAccessorIdentity;

    fn index(&self, id: PropertySetterId) -> &Self::Output {
        &self.setters[local_index(id)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessorTable {
    Getter,
    Setter,
}

impl AccessorTable {
    const fn role(self) -> AccessorRole {
        match self {
            Self::Getter => AccessorRole::Getter,
            Self::Setter => AccessorRole::Setter,
        }
    }
}

impl fmt::Display for AccessorTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Getter => "getter",
            Self::Setter => "setter",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPropertyAccessorIdentityTableError {
    Length {
        table: AccessorTable,
        expected: usize,
        actual: usize,
    },
    UnknownAccessor {
        table: AccessorTable,
        property: u32,
        accessor: u32,
    },
    DuplicateAccessor {
        table: AccessorTable,
        accessor: u32,
    },
    PropertyBackReference {
        table: AccessorTable,
        property: u32,
        accessor: u32,
        actual_property: u32,
    },
    Role {
        table: AccessorTable,
        accessor: u32,
    },
    Owner {
        table: AccessorTable,
        accessor: u32,
    },
    UnownedAccessor {
        table: AccessorTable,
        accessor: u32,
    },
}

impl fmt::Display for HirPropertyAccessorIdentityTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length {
                table,
                expected,
                actual,
            } => write!(
                formatter,
                "property {table} identity table has {actual} entries, expected {expected}"
            ),
            Self::UnknownAccessor {
                table,
                property,
                accessor,
            } => write!(
                formatter,
                "property {property} refers to unknown {table} {accessor}"
            ),
            Self::DuplicateAccessor { table, accessor } => {
                write!(formatter, "property {table} {accessor} has multiple owners")
            }
            Self::PropertyBackReference {
                table,
                property,
                accessor,
                actual_property,
            } => write!(
                formatter,
                "property {property} refers to {table} {accessor}, which points to property {actual_property}"
            ),
            Self::Role { table, accessor } => {
                write!(formatter, "property {table} {accessor} has the wrong role")
            }
            Self::Owner { table, accessor } => {
                write!(formatter, "property {table} {accessor} has the wrong owner")
            }
            Self::UnownedAccessor { table, accessor } => {
                write!(
                    formatter,
                    "property {table} {accessor} has no logical property"
                )
            }
        }
    }
}

impl std::error::Error for HirPropertyAccessorIdentityTableError {}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[cfg(test)]
mod tests;
