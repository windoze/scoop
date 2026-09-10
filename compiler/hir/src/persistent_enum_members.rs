//! Persistent identities aligned with source enum variants and their fields.

use std::fmt;
use std::ops::Index;

use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CanonicalIdentifierError, CborIdentityRecord, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityError, EnumVariantIdentityKey,
    PersistentEnumVariantFieldId, PersistentEnumVariantId,
};

use crate::{EnumDecl, EnumVariantFieldRef, EnumVariantRef, HirNominalIdentities};

type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type FieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;

#[derive(Clone, Debug)]
struct HirEnumIdentityRow {
    variants: Vec<HirEnumVariantIdentityRow>,
}

#[derive(Clone, Debug)]
struct HirEnumVariantIdentityRow {
    variant: VariantRecord,
    fields: Vec<FieldRecord>,
}

/// Total persistent identity relation for every source enum variant and field.
#[derive(Clone, Debug)]
pub struct HirEnumMemberIdentities {
    enums: Vec<HirEnumIdentityRow>,
}

impl HirEnumMemberIdentities {
    pub fn from_declarations(
        enums: &Arena<EnumDecl>,
        nominal_identities: &HirNominalIdentities,
    ) -> Result<Self, HirEnumMemberIdentityError> {
        let mut rows = Vec::with_capacity(enums.len());
        for (enum_id, enumeration) in enums.iter() {
            let enum_index = raw_enum_index(enum_id);
            let owner = nominal_identities[enum_id].source().ok_or(
                HirEnumMemberIdentityError::GeneratedEnumOwner {
                    enumeration: enum_index,
                },
            )?;
            let mut variant_rows = Vec::with_capacity(enumeration.variants.len());
            for (variant_index, variant) in enumeration.variants.iter().enumerate() {
                let variant_index = u32::try_from(variant_index).map_err(|_| {
                    HirEnumMemberIdentityError::TooManyVariants {
                        enumeration: enum_index,
                    }
                })?;
                let name = CanonicalIdentifier::new(&variant.name).map_err(|error| {
                    HirEnumMemberIdentityError::InvalidVariantName {
                        enumeration: enum_index,
                        variant: variant_index,
                        error,
                    }
                })?;
                let key =
                    EnumVariantIdentityKey::source(owner.declaration(), name).map_err(|error| {
                        HirEnumMemberIdentityError::InvalidVariantIdentity {
                            enumeration: enum_index,
                            variant: variant_index,
                            error,
                        }
                    })?;
                let variant_record = CborIdentityRecord::from_key(key).map_err(|error| {
                    HirEnumMemberIdentityError::InvalidVariantIdentity {
                        enumeration: enum_index,
                        variant: variant_index,
                        error,
                    }
                })?;
                let mut field_records = Vec::with_capacity(variant.fields.len());
                for (field_index, field) in variant.fields.iter().enumerate() {
                    let field_index = u32::try_from(field_index).map_err(|_| {
                        HirEnumMemberIdentityError::TooManyFields {
                            enumeration: enum_index,
                            variant: variant_index,
                        }
                    })?;
                    let selector = match variant.style {
                        crate::VariantStyle::Unit => {
                            return Err(HirEnumMemberIdentityError::UnitVariantField {
                                enumeration: enum_index,
                                variant: variant_index,
                                field: field_index,
                            });
                        }
                        crate::VariantStyle::Positional => EnumVariantFieldSelector::Positional {
                            declaration_index: field_index,
                        },
                        crate::VariantStyle::Named | crate::VariantStyle::Constructor => {
                            EnumVariantFieldSelector::Named(
                                CanonicalIdentifier::new(&field.name).map_err(|error| {
                                    HirEnumMemberIdentityError::InvalidFieldName {
                                        enumeration: enum_index,
                                        variant: variant_index,
                                        field: field_index,
                                        error,
                                    }
                                })?,
                            )
                        }
                    };
                    let key = EnumVariantFieldKey::new(variant_record.id(), selector);
                    let record = CborIdentityRecord::from_key(key).map_err(|error| {
                        HirEnumMemberIdentityError::InvalidFieldIdentity {
                            enumeration: enum_index,
                            variant: variant_index,
                            field: field_index,
                            error,
                        }
                    })?;
                    field_records.push(record);
                }
                variant_rows.push(HirEnumVariantIdentityRow {
                    variant: variant_record,
                    fields: field_records,
                });
            }
            rows.push(HirEnumIdentityRow {
                variants: variant_rows,
            });
        }
        Ok(Self { enums: rows })
    }
}

impl Index<EnumVariantRef> for HirEnumMemberIdentities {
    type Output = VariantRecord;

    fn index(&self, variant: EnumVariantRef) -> &Self::Output {
        &self.enums[variant.enumeration().into_raw().into_u32() as usize].variants
            [variant.local_index() as usize]
            .variant
    }
}

impl Index<EnumVariantFieldRef> for HirEnumMemberIdentities {
    type Output = FieldRecord;

    fn index(&self, field: EnumVariantFieldRef) -> &Self::Output {
        &self.enums[field.variant().enumeration().into_raw().into_u32() as usize].variants
            [field.variant().local_index() as usize]
            .fields[field.local_index() as usize]
    }
}

fn raw_enum_index(id: crate::EnumId) -> u32 {
    id.into_raw().into_u32()
}

#[derive(Debug)]
pub enum HirEnumMemberIdentityError {
    GeneratedEnumOwner {
        enumeration: u32,
    },
    TooManyVariants {
        enumeration: u32,
    },
    TooManyFields {
        enumeration: u32,
        variant: u32,
    },
    InvalidVariantName {
        enumeration: u32,
        variant: u32,
        error: CanonicalIdentifierError,
    },
    InvalidVariantIdentity {
        enumeration: u32,
        variant: u32,
        error: EnumVariantIdentityError,
    },
    UnitVariantField {
        enumeration: u32,
        variant: u32,
        field: u32,
    },
    InvalidFieldName {
        enumeration: u32,
        variant: u32,
        field: u32,
        error: CanonicalIdentifierError,
    },
    InvalidFieldIdentity {
        enumeration: u32,
        variant: u32,
        field: u32,
        error: scoop_wire::HashError,
    },
}

impl HirEnumMemberIdentityError {
    pub const fn enumeration(&self) -> u32 {
        match self {
            Self::GeneratedEnumOwner { enumeration }
            | Self::TooManyVariants { enumeration }
            | Self::TooManyFields { enumeration, .. }
            | Self::InvalidVariantName { enumeration, .. }
            | Self::InvalidVariantIdentity { enumeration, .. }
            | Self::UnitVariantField { enumeration, .. }
            | Self::InvalidFieldName { enumeration, .. }
            | Self::InvalidFieldIdentity { enumeration, .. } => *enumeration,
        }
    }
}

impl fmt::Display for HirEnumMemberIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedEnumOwner { enumeration } => write!(
                formatter,
                "enum {enumeration} has a generated nominal identity but source variants"
            ),
            Self::TooManyVariants { enumeration } => {
                write!(
                    formatter,
                    "enum {enumeration} has more than u32::MAX variants"
                )
            }
            Self::TooManyFields {
                enumeration,
                variant,
            } => write!(
                formatter,
                "enum {enumeration} variant {variant} has more than u32::MAX fields"
            ),
            Self::InvalidVariantName {
                enumeration,
                variant,
                error,
            } => write!(
                formatter,
                "enum {enumeration} variant {variant} has an invalid persistent name: {error}"
            ),
            Self::InvalidVariantIdentity {
                enumeration,
                variant,
                error,
            } => write!(
                formatter,
                "enum {enumeration} variant {variant} has an invalid persistent identity: {error}"
            ),
            Self::UnitVariantField {
                enumeration,
                variant,
                field,
            } => write!(
                formatter,
                "enum {enumeration} unit variant {variant} unexpectedly contains field {field}"
            ),
            Self::InvalidFieldName {
                enumeration,
                variant,
                field,
                error,
            } => write!(
                formatter,
                "enum {enumeration} variant {variant} field {field} has an invalid persistent name: {error}"
            ),
            Self::InvalidFieldIdentity {
                enumeration,
                variant,
                field,
                error,
            } => write!(
                formatter,
                "enum {enumeration} variant {variant} field {field} has an invalid persistent identity: {error}"
            ),
        }
    }
}

impl std::error::Error for HirEnumMemberIdentityError {}

#[cfg(test)]
mod tests;
