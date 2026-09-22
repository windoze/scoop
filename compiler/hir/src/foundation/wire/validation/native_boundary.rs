use std::collections::HashSet;
use std::fmt;

use scoop_identity::{
    CborIdentityRecord, EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::HirFoundationValidationError;
use crate::{
    NativeBoundaryNominalOwner, NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
};

mod graph;
pub(super) use graph::validate_graph_shape_coverage;

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type VariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;

pub(super) fn validate_shape_coverage(
    fields: &[FieldRecord],
    variants: &[VariantRecord],
    variant_fields: &[VariantFieldRecord],
    definitions: &[NativeBoundaryTypeDefinitionRecord],
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationValidationError> {
    let field_path = WirePath::root().field(12);
    let mut source_fields = HashSet::new();
    let source_field_count = fields
        .iter()
        .filter(|field| field.key().source_owner().is_some())
        .count();
    meter
        .try_reserve_set_slots(&mut source_fields, source_field_count, &field_path)
        .map_err(HirFoundationValidationError::Resource)?;
    source_fields.extend(
        fields
            .iter()
            .filter_map(|field| field.key().source_owner().map(|owner| (owner, field.id()))),
    );

    let variant_path = WirePath::root().field(13);
    let mut source_variants = HashSet::new();
    let source_variant_count = variants
        .iter()
        .filter(|variant| variant.key().source_owner().is_some())
        .count();
    meter
        .try_reserve_set_slots(&mut source_variants, source_variant_count, &variant_path)
        .map_err(HirFoundationValidationError::Resource)?;
    source_variants.extend(variants.iter().filter_map(|variant| {
        variant
            .key()
            .source_owner()
            .map(|owner| (owner, variant.id()))
    }));

    let variant_field_path = WirePath::root().field(14);
    let mut source_variant_fields = HashSet::new();
    meter
        .try_reserve_set_slots(
            &mut source_variant_fields,
            variant_fields.len(),
            &variant_field_path,
        )
        .map_err(HirFoundationValidationError::Resource)?;
    source_variant_fields.extend(
        variant_fields
            .iter()
            .map(|field| (field.key().variant(), field.id())),
    );

    let actual_field_count = checked_sum(
        definitions
            .iter()
            .filter_map(|definition| match definition.shape() {
                NativeBoundaryNominalShape::Struct { fields, .. } => Some(fields.len()),
                NativeBoundaryNominalShape::Reference
                | NativeBoundaryNominalShape::Intrinsic(_)
                | NativeBoundaryNominalShape::Enum { .. } => None,
            }),
        &WirePath::root().field(34),
    )?;
    let actual_variant_count = checked_sum(
        definitions
            .iter()
            .filter_map(|definition| match definition.shape() {
                NativeBoundaryNominalShape::Enum { variants } => Some(variants.len()),
                NativeBoundaryNominalShape::Reference
                | NativeBoundaryNominalShape::Intrinsic(_)
                | NativeBoundaryNominalShape::Struct { .. } => None,
            }),
        &WirePath::root().field(34),
    )?;
    let actual_variant_field_count = checked_sum(
        definitions
            .iter()
            .filter_map(|definition| match definition.shape() {
                NativeBoundaryNominalShape::Enum { variants } => Some(variants.as_slice()),
                NativeBoundaryNominalShape::Reference
                | NativeBoundaryNominalShape::Intrinsic(_)
                | NativeBoundaryNominalShape::Struct { .. } => None,
            })
            .flatten()
            .map(|variant| variant.fields().len()),
        &WirePath::root().field(34),
    )?;
    let definition_path = WirePath::root().field(34);
    let mut actual_fields = HashSet::new();
    meter
        .try_reserve_set_slots(&mut actual_fields, actual_field_count, &definition_path)
        .map_err(HirFoundationValidationError::Resource)?;
    let mut actual_variants = HashSet::new();
    meter
        .try_reserve_set_slots(&mut actual_variants, actual_variant_count, &definition_path)
        .map_err(HirFoundationValidationError::Resource)?;
    let mut actual_variant_fields = HashSet::new();
    meter
        .try_reserve_set_slots(
            &mut actual_variant_fields,
            actual_variant_field_count,
            &definition_path,
        )
        .map_err(HirFoundationValidationError::Resource)?;

    for (definition_index, definition) in definitions.iter().enumerate() {
        let owner = definition.owner();
        let declaration_owner = owner.declaration_owner();
        match definition.shape() {
            NativeBoundaryNominalShape::Reference => continue,
            NativeBoundaryNominalShape::Intrinsic(_) => {
                if let Some(field) = fields
                    .iter()
                    .filter(|field| field.key().source_owner() == Some(declaration_owner))
                    .map(CborIdentityRecord::id)
                    .min()
                {
                    return Err(NativeBoundaryShapeCoverageError::UnexpectedIntrinsicField {
                        owner,
                        field,
                    }
                    .into());
                }
            }
            NativeBoundaryNominalShape::Struct {
                fields: definition_fields,
                ..
            } => {
                actual_fields.extend(
                    definition_fields
                        .iter()
                        .map(|field| (definition_index, field.field())),
                );
                if let Some(field) = fields
                    .iter()
                    .filter(|field| field.key().source_owner() == Some(declaration_owner))
                    .map(CborIdentityRecord::id)
                    .filter(|field| !actual_fields.contains(&(definition_index, *field)))
                    .min()
                {
                    return Err(NativeBoundaryShapeCoverageError::MissingStructField {
                        owner,
                        field,
                    }
                    .into());
                }
                if let Some(field) = definition_fields
                    .iter()
                    .map(|field| field.field())
                    .filter(|field| !source_fields.contains(&(declaration_owner, *field)))
                    .min()
                {
                    return Err(NativeBoundaryShapeCoverageError::UnexpectedStructField {
                        owner,
                        field,
                    }
                    .into());
                }
            }
            NativeBoundaryNominalShape::Enum {
                variants: definition_variants,
            } => {
                actual_variants.extend(
                    definition_variants
                        .iter()
                        .map(|variant| (definition_index, variant.variant())),
                );
                if let Some(variant) = variants
                    .iter()
                    .filter(|variant| variant.key().source_owner() == Some(declaration_owner))
                    .map(CborIdentityRecord::id)
                    .filter(|variant| !actual_variants.contains(&(definition_index, *variant)))
                    .min()
                {
                    return Err(NativeBoundaryShapeCoverageError::MissingEnumVariant {
                        owner,
                        variant,
                    }
                    .into());
                }
                if let Some(variant) = definition_variants
                    .iter()
                    .map(|variant| variant.variant())
                    .filter(|variant| !source_variants.contains(&(declaration_owner, *variant)))
                    .min()
                {
                    return Err(NativeBoundaryShapeCoverageError::UnexpectedEnumVariant {
                        owner,
                        variant,
                    }
                    .into());
                }

                for (variant_index, variant) in definition_variants.iter().enumerate() {
                    actual_variant_fields.extend(
                        variant
                            .fields()
                            .iter()
                            .map(|field| (definition_index, variant_index, field.field())),
                    );
                    if let Some(field) = variant_fields
                        .iter()
                        .filter(|field| field.key().variant() == variant.variant())
                        .map(CborIdentityRecord::id)
                        .filter(|field| {
                            !actual_variant_fields.contains(&(
                                definition_index,
                                variant_index,
                                *field,
                            ))
                        })
                        .min()
                    {
                        return Err(NativeBoundaryShapeCoverageError::MissingEnumVariantField {
                            variant: variant.variant(),
                            field,
                        }
                        .into());
                    }
                    if let Some(field) = variant
                        .fields()
                        .iter()
                        .map(|field| field.field())
                        .filter(|field| {
                            !source_variant_fields.contains(&(variant.variant(), *field))
                        })
                        .min()
                    {
                        return Err(
                            NativeBoundaryShapeCoverageError::UnexpectedEnumVariantField {
                                variant: variant.variant(),
                                field,
                            }
                            .into(),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn checked_sum(
    counts: impl IntoIterator<Item = usize>,
    path: &WirePath,
) -> Result<usize, HirFoundationValidationError> {
    let count = counts.into_iter().try_fold(0_u64, |total, count| {
        let count = u64::try_from(count).map_err(|_| {
            HirFoundationValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        total.checked_add(count).ok_or_else(|| {
            HirFoundationValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })
    })?;
    usize::try_from(count).map_err(|_| {
        HirFoundationValidationError::Resource(WireError::new(
            WireErrorKind::IntegerOutOfRange,
            path.clone(),
            None,
        ))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeBoundaryShapeCoverageError {
    UnexpectedIntrinsicField {
        owner: NativeBoundaryNominalOwner,
        field: PersistentFieldId,
    },
    MissingStructField {
        owner: NativeBoundaryNominalOwner,
        field: PersistentFieldId,
    },
    UnexpectedStructField {
        owner: NativeBoundaryNominalOwner,
        field: PersistentFieldId,
    },
    MissingEnumVariant {
        owner: NativeBoundaryNominalOwner,
        variant: PersistentEnumVariantId,
    },
    UnexpectedEnumVariant {
        owner: NativeBoundaryNominalOwner,
        variant: PersistentEnumVariantId,
    },
    MissingEnumVariantField {
        variant: PersistentEnumVariantId,
        field: PersistentEnumVariantFieldId,
    },
    UnexpectedEnumVariantField {
        variant: PersistentEnumVariantId,
        field: PersistentEnumVariantFieldId,
    },
}

impl fmt::Display for NativeBoundaryShapeCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedIntrinsicField { owner, field } => write!(
                formatter,
                "native intrinsic {owner:?} contains source field {field}"
            ),
            Self::MissingStructField { owner, field } => write!(
                formatter,
                "native boundary {owner:?} omits source struct field {field}"
            ),
            Self::UnexpectedStructField { owner, field } => write!(
                formatter,
                "native boundary {owner:?} contains non-source struct field {field}"
            ),
            Self::MissingEnumVariant { owner, variant } => write!(
                formatter,
                "native boundary {owner:?} omits source enum variant {variant}"
            ),
            Self::UnexpectedEnumVariant { owner, variant } => write!(
                formatter,
                "native boundary {owner:?} contains non-source enum variant {variant}"
            ),
            Self::MissingEnumVariantField { variant, field } => write!(
                formatter,
                "native boundary enum variant {variant} omits source field {field}"
            ),
            Self::UnexpectedEnumVariantField { variant, field } => write!(
                formatter,
                "native boundary enum variant {variant} contains non-source field {field}"
            ),
        }
    }
}

impl std::error::Error for NativeBoundaryShapeCoverageError {}

#[cfg(test)]
mod tests;
