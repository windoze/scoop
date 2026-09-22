use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    PackagePath, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
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
    let field_key =
        FieldIdentityKey::source_declared(&declaration, CanonicalIdentifier::new("first").unwrap())
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
                scoop_identity::PersistentTypeId::from_source_declaration(&declaration).unwrap()
            ),
            field: field.id(),
        })
    );
}

#[test]
fn accepts_the_exact_source_struct_field_set() {
    let declaration = source_struct("Pair");
    let field_key =
        FieldIdentityKey::source_declared(&declaration, CanonicalIdentifier::new("first").unwrap())
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
    let field_key =
        FieldIdentityKey::source_declared(&declaration, CanonicalIdentifier::new("first").unwrap())
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
    let variant_key =
        EnumVariantIdentityKey::source(&declaration, CanonicalIdentifier::new("First").unwrap())
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
                scoop_identity::PersistentTypeId::from_source_declaration(&declaration).unwrap()
            ),
            variant: variant.id(),
        })
    );
}

#[test]
fn rejects_a_native_enum_witness_that_omits_a_variant_field() {
    let declaration = source_enum("Choice");
    let variant_key =
        EnumVariantIdentityKey::source(&declaration, CanonicalIdentifier::new("First").unwrap())
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
            variants: vec![NativeBoundaryVariantDefinition::new(&variant_key, Vec::new()).unwrap()],
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

#[test]
fn intrinsic_witness_cannot_hide_source_fields() {
    let declaration = source_struct("ScalarWithField");
    let field = CborIdentityRecord::from_key(
        FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("hidden").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let definition = NativeBoundaryTypeDefinitionRecord::new(
        &declaration,
        &[0],
        NativeBoundaryNominalShape::Intrinsic(crate::NominalIntrinsicRepresentationV1::new(
            crate::IntrinsicTypeKind::Boolean,
        )),
    )
    .unwrap();
    let owner = definition.owner();
    assert_eq!(
        validate_shape(std::slice::from_ref(&field), &[], &[], &[definition]),
        Err(NativeBoundaryShapeCoverageError::UnexpectedIntrinsicField {
            owner,
            field: field.id()
        })
    );
}
