//! Persistent identities aligned with the export HIR property arena.

use std::fmt;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CborIdentityRecord, DefinitionOwnerAtom, PersistentExtensionPropertyId, PersistentPropertyId,
    SourceDeclarationIdentityError, SourceDeclarationKey,
};

use crate::{ExtensionProperty, Property, PropertyId, PropertyOwner};

/// A source property has exactly one persistent identity kind, selected by
/// whether it declares an extension receiver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirPropertyIdentity {
    Ordinary(CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>),
    Extension(CborIdentityRecord<PersistentExtensionPropertyId, SourceDeclarationKey>),
}

impl HirPropertyIdentity {
    pub fn from_ordinary_declaration(
        declaration: SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        CborIdentityRecord::from_key(declaration).map(Self::Ordinary)
    }

    pub fn from_extension_declaration(
        declaration: SourceDeclarationKey,
    ) -> Result<Self, SourceDeclarationIdentityError> {
        CborIdentityRecord::from_key(declaration).map(Self::Extension)
    }

    pub fn declaration(&self) -> &SourceDeclarationKey {
        match self {
            Self::Ordinary(record) => record.key(),
            Self::Extension(record) => record.key(),
        }
    }

    pub const fn ordinary_id(&self) -> Option<PersistentPropertyId> {
        match self {
            Self::Ordinary(record) => Some(record.id()),
            Self::Extension(_) => None,
        }
    }

    pub const fn extension_id(&self) -> Option<PersistentExtensionPropertyId> {
        match self {
            Self::Ordinary(_) => None,
            Self::Extension(record) => Some(record.id()),
        }
    }

    pub const fn definition_owner(&self) -> DefinitionOwnerAtom {
        match self {
            Self::Ordinary(record) => DefinitionOwnerAtom::Property(record.id()),
            Self::Extension(record) => DefinitionOwnerAtom::ExtensionProperty(record.id()),
        }
    }
}

/// Total persistent identity relation for the export HIR property arena.
#[derive(Clone, Debug)]
pub struct HirPropertyIdentities {
    identities: Vec<HirPropertyIdentity>,
}

impl HirPropertyIdentities {
    pub fn checked(
        properties: &Arena<Property>,
        identities: Vec<HirPropertyIdentity>,
        extension_properties: &Arena<ExtensionProperty>,
    ) -> Result<Self, HirPropertyIdentityTableError> {
        if properties.len() != identities.len() {
            return Err(HirPropertyIdentityTableError::Length {
                expected: properties.len(),
                actual: identities.len(),
            });
        }

        for (property_id, property) in properties.iter() {
            let identity = &identities[local_index(property_id)];
            match (property.owner, identity) {
                (PropertyOwner::Extension(extension), HirPropertyIdentity::Extension(_)) => {
                    let index = local_index(extension);
                    if index >= extension_properties.len() {
                        return Err(HirPropertyIdentityTableError::UnknownExtension {
                            property: raw_index(property_id),
                            extension: raw_index(extension),
                        });
                    }
                    if extension_properties[extension].property != property_id {
                        return Err(HirPropertyIdentityTableError::ExtensionBackReference {
                            property: raw_index(property_id),
                            extension: raw_index(extension),
                            actual_property: raw_index(extension_properties[extension].property),
                        });
                    }
                }
                (PropertyOwner::Extension(_), HirPropertyIdentity::Ordinary(_))
                | (
                    PropertyOwner::TopLevel
                    | PropertyOwner::Class(_)
                    | PropertyOwner::Struct(_)
                    | PropertyOwner::Enum(_)
                    | PropertyOwner::Interface(_)
                    | PropertyOwner::Object(_),
                    HirPropertyIdentity::Extension(_),
                ) => {
                    return Err(HirPropertyIdentityTableError::Kind {
                        property: raw_index(property_id),
                    });
                }
                (
                    PropertyOwner::TopLevel
                    | PropertyOwner::Class(_)
                    | PropertyOwner::Struct(_)
                    | PropertyOwner::Enum(_)
                    | PropertyOwner::Interface(_)
                    | PropertyOwner::Object(_),
                    HirPropertyIdentity::Ordinary(_),
                ) => {}
            }
        }

        for (extension_id, extension) in extension_properties.iter() {
            let property_index = local_index(extension.property);
            if property_index >= properties.len() {
                return Err(HirPropertyIdentityTableError::UnknownProperty {
                    extension: raw_index(extension_id),
                    property: raw_index(extension.property),
                });
            }
            if properties[extension.property].owner != PropertyOwner::Extension(extension_id) {
                return Err(HirPropertyIdentityTableError::PropertyBackReference {
                    extension: raw_index(extension_id),
                    property: raw_index(extension.property),
                });
            }
        }

        Ok(Self { identities })
    }
}

impl Index<PropertyId> for HirPropertyIdentities {
    type Output = HirPropertyIdentity;

    fn index(&self, id: PropertyId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPropertyIdentityTableError {
    Length {
        expected: usize,
        actual: usize,
    },
    Kind {
        property: u32,
    },
    UnknownExtension {
        property: u32,
        extension: u32,
    },
    ExtensionBackReference {
        property: u32,
        extension: u32,
        actual_property: u32,
    },
    UnknownProperty {
        extension: u32,
        property: u32,
    },
    PropertyBackReference {
        extension: u32,
        property: u32,
    },
}

impl fmt::Display for HirPropertyIdentityTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => {
                write!(
                    formatter,
                    "property identity table has {actual} entries, expected {expected}"
                )
            }
            Self::Kind { property } => {
                write!(
                    formatter,
                    "property {property} has the wrong persistent identity kind"
                )
            }
            Self::UnknownExtension {
                property,
                extension,
            } => write!(
                formatter,
                "property {property} refers to unknown extension property {extension}"
            ),
            Self::ExtensionBackReference {
                property,
                extension,
                actual_property,
            } => write!(
                formatter,
                "property {property} refers to extension property {extension}, which points to property {actual_property}"
            ),
            Self::UnknownProperty {
                extension,
                property,
            } => write!(
                formatter,
                "extension property {extension} refers to unknown property {property}"
            ),
            Self::PropertyBackReference {
                extension,
                property,
            } => write!(
                formatter,
                "extension property {extension} refers to property {property} with a different owner"
            ),
        }
    }
}

impl std::error::Error for HirPropertyIdentityTableError {}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[cfg(test)]
mod tests;
