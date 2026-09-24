use scoop_identity::{
    EnumVariantFieldSelector, PersistentTypeId, SignatureTypeKey, SourceNominalKind,
};

use super::*;
use crate::{
    EnumSourceShapeV1, ObjectSourceShapeV1, SignatureBinderScopeError, StructSourceShapeV1,
};

mod reference_fields;
mod support;

use support::*;

#[test]
fn validates_owned_source_shapes_selectors_and_field_types() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let binders = binders();

    let structure = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(fixture.struct_field, binder(0))],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    assert_eq!(
        structure.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders,
            &mut authority,
        ),
        Ok(())
    );

    let enumeration = NominalSourceShapeV1::Enum(
        EnumSourceShapeV1::try_new(vec![
            variant(
                fixture.positional_variant,
                EnumSourceVariantStyleV1::Positional,
                vec![enum_field(fixture.positional_field, 0)],
            ),
            variant(
                fixture.named_variant,
                EnumSourceVariantStyleV1::Named,
                vec![enum_field(fixture.named_field, 0)],
            ),
            variant(
                fixture.constructor_variant,
                EnumSourceVariantStyleV1::Constructor,
                vec![enum_field(fixture.constructor_field, 0)],
            ),
            variant(
                fixture.unit_variant,
                EnumSourceVariantStyleV1::Unit,
                Vec::new(),
            ),
        ])
        .unwrap(),
    );
    assert_eq!(
        enumeration.validate_semantics(
            fixture.enum_owner,
            PublicNominalKindV1::Enum,
            &binders,
            &mut authority,
        ),
        Ok(())
    );

    let object = NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
        fixture.object_value,
        Default::default(),
    ));
    assert_eq!(
        object.validate_semantics(
            fixture.object_owner,
            PublicNominalKindV1::Object,
            &empty_binders(),
            &mut authority,
        ),
        Ok(())
    );
}

#[test]
fn rejects_shape_kind_mismatch() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();

    assert_eq!(
        NominalSourceShapeV1::Class(Default::default()).validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders(),
            &mut authority,
        ),
        Err(NominalSourceShapeSemanticError::Kind {
            expected: PublicNominalKindV1::Struct,
            actual: PublicNominalKindV1::Class,
        })
    );
}

#[test]
fn rejects_fields_variants_and_object_values_owned_by_other_nominals() {
    let fixture = Fixture::new();

    let structure = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(
                fixture.foreign_struct_field,
                binder(0),
            )],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    let mut struct_authority = fixture.authority();
    assert!(matches!(
        structure.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders(),
            &mut struct_authority,
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            index: 0,
            error: NominalSourceFieldSemanticError::Owner {
                expected,
                actual: Some(actual),
            },
        }) if expected == fixture.struct_owner && actual == fixture.foreign_struct_owner
    ));

    let enumeration = NominalSourceShapeV1::Enum(
        EnumSourceShapeV1::try_new(vec![variant(
            fixture.foreign_variant,
            EnumSourceVariantStyleV1::Unit,
            Vec::new(),
        )])
        .unwrap(),
    );
    let mut enum_authority = fixture.authority();
    assert!(matches!(
        enumeration.validate_semantics(
            fixture.enum_owner,
            PublicNominalKindV1::Enum,
            &binders(),
            &mut enum_authority,
        ),
        Err(NominalSourceShapeSemanticError::EnumVariant {
            index: 0,
            error: EnumSourceVariantSemanticError::Owner {
                expected,
                actual: Some(actual),
            },
        }) if expected == fixture.enum_owner && actual == fixture.foreign_enum_owner
    ));

    let object = NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
        fixture.foreign_object_value,
        Default::default(),
    ));
    let mut object_authority = fixture.authority();
    assert!(matches!(
        object.validate_semantics(
            fixture.object_owner,
            PublicNominalKindV1::Object,
            &empty_binders(),
            &mut object_authority,
        ),
        Err(NominalSourceShapeSemanticError::ObjectValue(
            ObjectSourceShapeSemanticError::Owner { expected, actual }
        )) if expected == fixture.object_owner && actual == fixture.foreign_object_owner
    ));
}

#[test]
fn rejects_variant_field_owner_and_selector_mismatches() {
    let fixture = Fixture::new();

    let wrong_variant = enum_shape(
        fixture.positional_variant,
        EnumSourceVariantStyleV1::Positional,
        fixture.foreign_variant_field,
    );
    let mut wrong_variant_authority = fixture.authority();
    assert!(matches!(
        wrong_variant.validate_semantics(
            fixture.enum_owner,
            PublicNominalKindV1::Enum,
            &binders(),
            &mut wrong_variant_authority,
        ),
        Err(NominalSourceShapeSemanticError::EnumVariant {
            index: 0,
            error: EnumSourceVariantSemanticError::Field {
                index: 0,
                error: EnumSourceFieldSemanticError::Variant { expected, actual },
            },
        }) if expected == fixture.positional_variant && actual == fixture.foreign_variant
    ));

    let wrong_position = enum_shape(
        fixture.positional_variant,
        EnumSourceVariantStyleV1::Positional,
        fixture.positional_field_one,
    );
    let mut wrong_position_authority = fixture.authority();
    assert!(matches!(
        wrong_position.validate_semantics(
            fixture.enum_owner,
            PublicNominalKindV1::Enum,
            &binders(),
            &mut wrong_position_authority,
        ),
        Err(NominalSourceShapeSemanticError::EnumVariant {
            error: EnumSourceVariantSemanticError::Field {
                index: 0,
                error: EnumSourceFieldSemanticError::Selector {
                    expected: EnumSourceFieldSelectorV1::Positional {
                        declaration_index: 0,
                    },
                    actual: EnumVariantFieldSelector::Positional {
                        declaration_index: 1,
                    },
                },
            },
            ..
        })
    ));

    let positional_in_named = enum_shape(
        fixture.positional_variant,
        EnumSourceVariantStyleV1::Named,
        fixture.positional_field,
    );
    let mut named_authority = fixture.authority();
    assert!(matches!(
        positional_in_named.validate_semantics(
            fixture.enum_owner,
            PublicNominalKindV1::Enum,
            &binders(),
            &mut named_authority,
        ),
        Err(NominalSourceShapeSemanticError::EnumVariant {
            error: EnumSourceVariantSemanticError::Field {
                error: EnumSourceFieldSemanticError::Selector {
                    expected: EnumSourceFieldSelectorV1::Named,
                    actual: EnumVariantFieldSelector::Positional {
                        declaration_index: 0,
                    },
                },
                ..
            },
            ..
        })
    ));
}

#[test]
fn rejects_out_of_scope_and_unresolved_field_types() {
    let fixture = Fixture::new();
    let out_of_scope = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(fixture.struct_field, binder(1))],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    let mut scope_authority = fixture.authority();
    assert!(matches!(
        out_of_scope.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders(),
            &mut scope_authority,
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            index: 0,
            error: NominalSourceFieldSemanticError::ValueType(
                SignatureTypeSemanticError::BinderScope(
                    SignatureBinderScopeError::IndexOutOfRange {
                        depth: 0,
                        index: 1,
                        arity: 1,
                    }
                )
            ),
        })
    ));

    let missing = nominal("Missing", SourceNominalKind::Class, 0);
    let missing_id = PersistentTypeId::from_source_declaration(&missing).unwrap();
    let unresolved = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(
                fixture.struct_field,
                SignatureTypeKey::Nominal(missing_id),
            )],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    let mut reference_authority = fixture.authority();
    assert!(matches!(
        unresolved.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders(),
            &mut reference_authority,
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            error: NominalSourceFieldSemanticError::ValueType(
                SignatureTypeSemanticError::Reference(TestAuthorityError::Concrete(id))
            ),
            ..
        }) if id == missing_id
    ));
}

#[test]
fn rejects_missing_kind_specific_identity_authority() {
    let fixture = Fixture::new();
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(fixture.struct_field, binder(0))],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    let mut authority = fixture.authority();
    authority.fields.remove(&fixture.struct_field);

    assert!(matches!(
        shape.validate_semantics(
            fixture.struct_owner,
            PublicNominalKindV1::Struct,
            &binders(),
            &mut authority,
        ),
        Err(NominalSourceShapeSemanticError::NominalField {
            index: 0,
            error: NominalSourceFieldSemanticError::Reference(
                TestAuthorityError::Field(field)
            ),
        }) if field == fixture.struct_field
    ));
}
