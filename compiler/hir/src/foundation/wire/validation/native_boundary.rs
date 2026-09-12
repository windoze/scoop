use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CborIdentityRecord, EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey,
    NominalDeclarationOwner, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentFieldId,
};

use crate::{
    NativeBoundaryNominalOwner, NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
};

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type VariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;

pub(super) fn validate_shape_coverage(
    fields: &[FieldRecord],
    variants: &[VariantRecord],
    variant_fields: &[VariantFieldRecord],
    definitions: &[NativeBoundaryTypeDefinitionRecord],
) -> Result<(), NativeBoundaryShapeCoverageError> {
    let fields_by_owner = group_fields(fields);
    let variants_by_owner = group_variants(variants);
    let fields_by_variant = group_variant_fields(variant_fields);

    for definition in definitions {
        match definition.shape() {
            NativeBoundaryNominalShape::Reference => {}
            NativeBoundaryNominalShape::Struct { fields, .. } => {
                let actual = fields
                    .iter()
                    .map(|field| field.field())
                    .collect::<BTreeSet<_>>();
                let expected = fields_by_owner
                    .get(&definition.owner().declaration_owner())
                    .cloned()
                    .unwrap_or_default();
                require_exact_fields(definition.owner(), &expected, &actual)?;
            }
            NativeBoundaryNominalShape::Enum { variants } => {
                let actual = variants
                    .iter()
                    .map(|variant| variant.variant())
                    .collect::<BTreeSet<_>>();
                let expected = variants_by_owner
                    .get(&definition.owner().declaration_owner())
                    .cloned()
                    .unwrap_or_default();
                require_exact_variants(definition.owner(), &expected, &actual)?;

                for variant in variants {
                    let actual = variant
                        .fields()
                        .iter()
                        .map(|field| field.field())
                        .collect::<BTreeSet<_>>();
                    let expected = fields_by_variant
                        .get(&variant.variant())
                        .cloned()
                        .unwrap_or_default();
                    require_exact_variant_fields(variant.variant(), &expected, &actual)?;
                }
            }
        }
    }
    Ok(())
}

fn group_fields(
    fields: &[FieldRecord],
) -> BTreeMap<NominalDeclarationOwner, BTreeSet<PersistentFieldId>> {
    let mut result = BTreeMap::new();
    for field in fields {
        if let Some(owner) = field.key().source_owner() {
            result
                .entry(owner)
                .or_insert_with(BTreeSet::new)
                .insert(field.id());
        }
    }
    result
}

fn group_variants(
    variants: &[VariantRecord],
) -> BTreeMap<NominalDeclarationOwner, BTreeSet<PersistentEnumVariantId>> {
    let mut result = BTreeMap::new();
    for variant in variants {
        if let Some(owner) = variant.key().source_owner() {
            result
                .entry(owner)
                .or_insert_with(BTreeSet::new)
                .insert(variant.id());
        }
    }
    result
}

fn group_variant_fields(
    fields: &[VariantFieldRecord],
) -> BTreeMap<PersistentEnumVariantId, BTreeSet<PersistentEnumVariantFieldId>> {
    let mut result = BTreeMap::new();
    for field in fields {
        result
            .entry(field.key().variant())
            .or_insert_with(BTreeSet::new)
            .insert(field.id());
    }
    result
}

fn require_exact_fields(
    owner: NativeBoundaryNominalOwner,
    expected: &BTreeSet<PersistentFieldId>,
    actual: &BTreeSet<PersistentFieldId>,
) -> Result<(), NativeBoundaryShapeCoverageError> {
    if let Some(field) = expected.difference(actual).next() {
        return Err(NativeBoundaryShapeCoverageError::MissingStructField {
            owner,
            field: *field,
        });
    }
    if let Some(field) = actual.difference(expected).next() {
        return Err(NativeBoundaryShapeCoverageError::UnexpectedStructField {
            owner,
            field: *field,
        });
    }
    Ok(())
}

fn require_exact_variants(
    owner: NativeBoundaryNominalOwner,
    expected: &BTreeSet<PersistentEnumVariantId>,
    actual: &BTreeSet<PersistentEnumVariantId>,
) -> Result<(), NativeBoundaryShapeCoverageError> {
    if let Some(variant) = expected.difference(actual).next() {
        return Err(NativeBoundaryShapeCoverageError::MissingEnumVariant {
            owner,
            variant: *variant,
        });
    }
    if let Some(variant) = actual.difference(expected).next() {
        return Err(NativeBoundaryShapeCoverageError::UnexpectedEnumVariant {
            owner,
            variant: *variant,
        });
    }
    Ok(())
}

fn require_exact_variant_fields(
    variant: PersistentEnumVariantId,
    expected: &BTreeSet<PersistentEnumVariantFieldId>,
    actual: &BTreeSet<PersistentEnumVariantFieldId>,
) -> Result<(), NativeBoundaryShapeCoverageError> {
    if let Some(field) = expected.difference(actual).next() {
        return Err(NativeBoundaryShapeCoverageError::MissingEnumVariantField {
            variant,
            field: *field,
        });
    }
    if let Some(field) = actual.difference(expected).next() {
        return Err(
            NativeBoundaryShapeCoverageError::UnexpectedEnumVariantField {
                variant,
                field: *field,
            },
        );
    }
    Ok(())
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

    use super::*;
    use crate::{
        NativeBoundaryCLayoutPolicy, NativeBoundaryFieldDefinition, NativeBoundaryVariantDefinition,
    };

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
            validate_shape_coverage(std::slice::from_ref(&field), &[], &[], &[definition]),
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

        validate_shape_coverage(&[field], &[], &[], &[definition]).unwrap();
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
            validate_shape_coverage(&[], std::slice::from_ref(&variant), &[], &[definition]),
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
            validate_shape_coverage(&[], &[variant], std::slice::from_ref(&field), &[definition],),
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
