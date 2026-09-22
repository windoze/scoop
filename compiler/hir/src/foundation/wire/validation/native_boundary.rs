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
                NativeBoundaryNominalShape::Reference | NativeBoundaryNominalShape::Enum { .. } => {
                    None
                }
            }),
        &WirePath::root().field(30),
    )?;
    let actual_variant_count = checked_sum(
        definitions
            .iter()
            .filter_map(|definition| match definition.shape() {
                NativeBoundaryNominalShape::Enum { variants } => Some(variants.len()),
                NativeBoundaryNominalShape::Reference
                | NativeBoundaryNominalShape::Struct { .. } => None,
            }),
        &WirePath::root().field(30),
    )?;
    let actual_variant_field_count = checked_sum(
        definitions
            .iter()
            .filter_map(|definition| match definition.shape() {
                NativeBoundaryNominalShape::Enum { variants } => Some(variants.as_slice()),
                NativeBoundaryNominalShape::Reference
                | NativeBoundaryNominalShape::Struct { .. } => None,
            })
            .flatten()
            .map(|variant| variant.fields().len()),
        &WirePath::root().field(30),
    )?;
    let definition_path = WirePath::root().field(30);
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
            NativeBoundaryNominalShape::Reference => {}
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
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantFieldSelector,
        EnumVariantIdentityKey, FieldIdentityKey, PackagePath, SourceDeclarationKey,
        SourceDeclarationSite, SourceNominalKind,
    };
    use scoop_wire::{BudgetMeter, DecodeLimits};

    use super::*;
    use crate::{
        NativeBoundaryCLayoutPolicy, NativeBoundaryFieldDefinition, NativeBoundaryVariantDefinition,
    };

    fn validate_shape(
        fields: &[FieldRecord],
        variants: &[VariantRecord],
        variant_fields: &[VariantFieldRecord],
        definitions: &[NativeBoundaryTypeDefinitionRecord],
    ) -> Result<(), NativeBoundaryShapeCoverageError> {
        match validate_shape_coverage(
            fields,
            variants,
            variant_fields,
            definitions,
            &mut BudgetMeter::new(DecodeLimits::default()),
        ) {
            Ok(()) => Ok(()),
            Err(HirFoundationValidationError::NativeBoundaryShapeCoverage(error)) => Err(error),
            Err(error) => panic!("unexpected validation error: {error}"),
        }
    }

    #[test]
    fn rejects_a_native_struct_witness_that_omits_a_source_field() {
        let declaration = source_struct("Pair");
        let field_key = FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("first").unwrap(),
        )
        .unwrap();
        let field = CborIdentityRecord::from_key(field_key).unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            validate_shape(std::slice::from_ref(&field), &[], &[], &[definition]),
            Err(NativeBoundaryShapeCoverageError::MissingStructField {
                owner: NativeBoundaryNominalOwner::Concrete(
                    scoop_identity::PersistentTypeId::from_source_declaration(&declaration)
                        .unwrap()
                ),
                field: field.id(),
            })
        );
    }

    #[test]
    fn accepts_the_exact_source_struct_field_set() {
        let declaration = source_struct("Pair");
        let field_key = FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("first").unwrap(),
        )
        .unwrap();
        let field = CborIdentityRecord::from_key(field_key.clone()).unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![
                    NativeBoundaryFieldDefinition::new(
                        &field_key,
                        scoop_identity::SignatureTypeKey::Nominal(
                            scoop_identity::CoreBuiltinNominal::Unit
                                .identity_record()
                                .id(),
                        ),
                    )
                    .unwrap(),
                ],
            },
        )
        .unwrap();

        validate_shape(&[field], &[], &[], &[definition]).unwrap();
    }

    #[test]
    fn shape_coverage_indexes_have_inclusive_heap_boundaries() {
        let declaration = source_struct("BudgetedPair");
        let field_key = FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("first").unwrap(),
        )
        .unwrap();
        let field = CborIdentityRecord::from_key(field_key.clone()).unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![
                    NativeBoundaryFieldDefinition::new(
                        &field_key,
                        scoop_identity::SignatureTypeKey::Nominal(
                            scoop_identity::CoreBuiltinNominal::Unit
                                .identity_record()
                                .id(),
                        ),
                    )
                    .unwrap(),
                ],
            },
        )
        .unwrap();
        let required = 2 * scoop_wire::budget::COLLECTION_ELEMENT_BYTES;

        for (limit, accepted) in [
            (required - 1, false),
            (required, true),
            (required + 1, true),
        ] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: limit,
                ..DecodeLimits::default()
            });
            let result = validate_shape_coverage(
                std::slice::from_ref(&field),
                &[],
                &[],
                std::slice::from_ref(&definition),
                &mut meter,
            );
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(matches!(
                    result.unwrap_err(),
                    HirFoundationValidationError::Resource(ref error)
                        if error.kind() == &WireErrorKind::LimitExceeded {
                            resource: scoop_wire::ResourceKind::LogicalHeapBytes,
                            limit,
                            observed: required,
                        }
                ));
            }
        }
    }

    #[test]
    fn rejects_a_native_enum_witness_that_omits_a_source_variant() {
        let declaration = source_enum("Choice");
        let variant_key = EnumVariantIdentityKey::source(
            &declaration,
            CanonicalIdentifier::new("First").unwrap(),
        )
        .unwrap();
        let variant = CborIdentityRecord::from_key(variant_key).unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Enum {
                variants: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            validate_shape(&[], std::slice::from_ref(&variant), &[], &[definition]),
            Err(NativeBoundaryShapeCoverageError::MissingEnumVariant {
                owner: NativeBoundaryNominalOwner::Concrete(
                    scoop_identity::PersistentTypeId::from_source_declaration(&declaration)
                        .unwrap()
                ),
                variant: variant.id(),
            })
        );
    }

    #[test]
    fn rejects_a_native_enum_witness_that_omits_a_variant_field() {
        let declaration = source_enum("Choice");
        let variant_key = EnumVariantIdentityKey::source(
            &declaration,
            CanonicalIdentifier::new("First").unwrap(),
        )
        .unwrap();
        let variant = CborIdentityRecord::from_key(variant_key.clone()).unwrap();
        let field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            variant.id(),
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Enum {
                variants: vec![
                    NativeBoundaryVariantDefinition::new(&variant_key, Vec::new()).unwrap(),
                ],
            },
        )
        .unwrap();

        assert_eq!(
            validate_shape(&[], &[variant], std::slice::from_ref(&field), &[definition],),
            Err(NativeBoundaryShapeCoverageError::MissingEnumVariantField {
                variant: field.key().variant(),
                field: field.id(),
            })
        );
    }

    fn source_struct(name: &str) -> SourceDeclarationKey {
        source_nominal(name, SourceNominalKind::Struct)
    }

    fn source_enum(name: &str) -> SourceDeclarationKey {
        source_nominal(name, SourceNominalKind::Enum)
    }

    fn source_nominal(name: &str, kind: SourceNominalKind) -> SourceDeclarationKey {
        SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            0,
        )
    }
}
