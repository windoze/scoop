use super::*;

#[test]
fn applied_field_and_variant_refs_close_owner_and_index_relations() {
    let mut types = Arena::new();
    let int = types.alloc(Type::Integer(IntegerKind::SIGNED_32));
    let string = types.alloc(Type::String);
    let parameter = TypeParamId::from_raw(0);
    let parameter_ty = types.alloc(Type::Param(parameter));
    let mut enums = Arena::new();
    let first_application = EnumApplicationId::from_raw(0.into());
    let choice = enums.alloc(enum_declaration(
        "Choice",
        first_application,
        vec![
            Variant {
                name: "Data".to_string(),
                style: VariantStyle::Named,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: parameter_ty,
                }],
            },
            Variant {
                name: "Empty".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            },
        ],
    ));
    enums[choice].type_params.push(TypeParamDecl {
        id: parameter,
        name: "T".to_string(),
        bounds: TypeParamBounds::Unconstrained,
        span: Span::new(0, 0),
    });
    let other_application = EnumApplicationId::from_raw(3.into());
    let other = enums.alloc(enum_declaration(
        "Other",
        other_application,
        vec![Variant {
            name: "Data".to_string(),
            style: VariantStyle::Unit,
            fields: Vec::new(),
        }],
    ));
    let nominal_identities = identities(&Arena::new(), &enums);
    let mut applications = Arena::new();
    let choice_self = applications.alloc(EnumApplication {
        template: nominal_identities[choice].declaration_id(),
        arguments: vec![parameter_ty],
        canonical_type: parameter_ty,
    });
    assert_eq!(choice_self, first_application);
    let choice_int = applications.alloc(EnumApplication {
        template: nominal_identities[choice].declaration_id(),
        arguments: vec![int],
        canonical_type: int,
    });
    let choice_string = applications.alloc(EnumApplication {
        template: nominal_identities[choice].declaration_id(),
        arguments: vec![string],
        canonical_type: string,
    });
    let other_application = applications.alloc(EnumApplication {
        template: nominal_identities[other].declaration_id(),
        arguments: Vec::new(),
        canonical_type: int,
    });

    let data = EnumVariantRef::checked(&enums, choice, 0).unwrap();
    let empty = EnumVariantRef::checked(&enums, choice, 1).unwrap();
    assert!(EnumVariantRef::checked(&enums, choice, 2).is_none());
    assert!(EnumVariantRef::checked(&enums, EnumId::from_raw(99.into()), 0).is_none());
    let data_field = EnumVariantFieldRef::checked(&enums, data, 0).unwrap();
    assert_eq!(data_field.variant(), data);
    assert_eq!(data_field.local_index(), 0);
    assert!(EnumVariantFieldRef::checked(&enums, data, 1).is_none());
    assert!(EnumVariantFieldRef::checked(&enums, empty, 0).is_none());
    let applied_int = AppliedEnumVariantRef::checked(
        &enums,
        &applications,
        &nominal_identities,
        choice_int,
        data,
    )
    .unwrap();
    let applied_string = AppliedEnumVariantRef::checked(
        &enums,
        &applications,
        &nominal_identities,
        choice_string,
        data,
    )
    .unwrap();
    assert_ne!(applied_int, applied_string);
    assert!(
        AppliedEnumVariantRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            other_application,
            data
        )
        .is_none()
    );
    assert!(
        AppliedEnumVariantRef::checked_index(
            &enums,
            &applications,
            &nominal_identities,
            choice_int,
            2
        )
        .is_none()
    );
    assert!(
        AppliedEnumVariantRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            EnumApplicationId::from_raw(99.into()),
            data
        )
        .is_none()
    );
    assert!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            applied_int,
            0
        )
        .is_some()
    );
    assert!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            applied_int,
            1
        )
        .is_none()
    );
    let applied_empty = AppliedEnumVariantRef::checked(
        &enums,
        &applications,
        &nominal_identities,
        choice_int,
        empty,
    )
    .unwrap();
    assert!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            applied_empty,
            0
        )
        .is_none()
    );

    let mut structs = Arena::new();
    let declared_application = StructApplicationId::from_raw(0.into());
    let declared = structs.alloc(struct_declaration(
        "Record",
        declared_application,
        StructRepresentation::Declared(vec![Field {
            name: "value".to_string(),
            ty: parameter_ty,
        }]),
    ));
    structs[declared].type_params.push(TypeParamDecl {
        id: parameter,
        name: "T".to_string(),
        bounds: TypeParamBounds::Unconstrained,
        span: Span::new(0, 0),
    });
    let intrinsic_application = StructApplicationId::from_raw(3.into());
    let intrinsic = structs.alloc(struct_declaration(
        "Intrinsic",
        intrinsic_application,
        StructRepresentation::Intrinsic(IntrinsicTypeKind::Boolean),
    ));
    let nominal_identities = identities(&structs, &enums);
    let mut struct_applications = Arena::new();
    let declared_self = struct_applications.alloc(StructApplication {
        template: nominal_identities[declared].declaration_id(),
        arguments: vec![parameter_ty],
        canonical_type: parameter_ty,
        representation: StructApplicationRepresentation::Declared,
    });
    assert_eq!(declared_self, declared_application);
    let declared_int = struct_applications.alloc(StructApplication {
        template: nominal_identities[declared].declaration_id(),
        arguments: vec![int],
        canonical_type: int,
        representation: StructApplicationRepresentation::Declared,
    });
    let declared_string = struct_applications.alloc(StructApplication {
        template: nominal_identities[declared].declaration_id(),
        arguments: vec![string],
        canonical_type: string,
        representation: StructApplicationRepresentation::Declared,
    });
    let intrinsic_application = struct_applications.alloc(StructApplication {
        template: nominal_identities[intrinsic].declaration_id(),
        arguments: Vec::new(),
        canonical_type: int,
        representation: StructApplicationRepresentation::Intrinsic(
            IntrinsicTypeRepresentation::Boolean,
        ),
    });
    let int_field = AppliedStructFieldRef::checked(
        &structs,
        &struct_applications,
        &nominal_identities,
        declared_int,
        0,
    )
    .expect("Record<Int>.value exists");
    let string_field = AppliedStructFieldRef::checked(
        &structs,
        &struct_applications,
        &nominal_identities,
        declared_string,
        0,
    )
    .expect("Record<String>.value exists");
    assert_ne!(int_field, string_field);
    assert!(
        AppliedStructFieldRef::checked(
            &structs,
            &struct_applications,
            &nominal_identities,
            declared_int,
            0
        )
        .is_some()
    );
    assert!(
        AppliedStructFieldRef::checked(
            &structs,
            &struct_applications,
            &nominal_identities,
            declared_int,
            1
        )
        .is_none()
    );
    assert!(
        AppliedStructFieldRef::checked(
            &structs,
            &struct_applications,
            &nominal_identities,
            intrinsic_application,
            0
        )
        .is_none()
    );
}

#[test]
fn applied_enum_refs_revalidate_coordinates_in_target_stores() {
    let mut types = Arena::new();
    let unit = types.alloc(Type::Unit);

    let mut enums = Arena::new();
    let choice = enums.alloc(enum_declaration(
        "Choice",
        EnumApplicationId::from_raw(0.into()),
        vec![
            Variant {
                name: "Data".to_string(),
                style: VariantStyle::Named,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: unit,
                }],
            },
            Variant {
                name: "Empty".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            },
        ],
    ));
    let other = enums.alloc(enum_declaration(
        "Other",
        EnumApplicationId::from_raw(1.into()),
        vec![Variant {
            name: "Data".to_string(),
            style: VariantStyle::Named,
            fields: vec![Field {
                name: "value".to_string(),
                ty: unit,
            }],
        }],
    ));
    let nominal_identities = identities(&Arena::new(), &enums);
    let mut applications = Arena::new();
    let choice_application = applications.alloc(EnumApplication {
        template: nominal_identities[choice].declaration_id(),
        arguments: Vec::new(),
        canonical_type: unit,
    });
    let other_application = applications.alloc(EnumApplication {
        template: nominal_identities[other].declaration_id(),
        arguments: Vec::new(),
        canonical_type: unit,
    });

    let mut foreign_enums = Arena::new();
    let foreign_choice = foreign_enums.alloc(enum_declaration(
        "ForeignChoice",
        EnumApplicationId::from_raw(0.into()),
        vec![
            Variant {
                name: "ForeignData".to_string(),
                style: VariantStyle::Named,
                fields: vec![
                    Field {
                        name: "first".to_string(),
                        ty: unit,
                    },
                    Field {
                        name: "second".to_string(),
                        ty: unit,
                    },
                ],
            },
            Variant {
                name: "ForeignEmpty".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            },
            Variant {
                name: "ForeignExtra".to_string(),
                style: VariantStyle::Named,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: unit,
                }],
            },
        ],
    ));
    foreign_enums.alloc(enum_declaration(
        "ForeignOther",
        EnumApplicationId::from_raw(2.into()),
        Vec::new(),
    ));
    let foreign_nominal_identities = identities(&Arena::new(), &foreign_enums);
    let mut foreign_applications = Arena::new();
    let foreign_same_application = foreign_applications.alloc(EnumApplication {
        template: foreign_nominal_identities[foreign_choice].declaration_id(),
        arguments: Vec::new(),
        canonical_type: unit,
    });
    let foreign_wrong_owner_application = foreign_applications.alloc(EnumApplication {
        template: foreign_nominal_identities[foreign_choice].declaration_id(),
        arguments: Vec::new(),
        canonical_type: unit,
    });

    assert_eq!(choice_application, foreign_same_application);
    assert_eq!(other_application, foreign_wrong_owner_application);
    let foreign_data = EnumVariantRef::checked(&foreign_enums, foreign_choice, 0).unwrap();
    let foreign_applied = AppliedEnumVariantRef::checked(
        &foreign_enums,
        &foreign_applications,
        &foreign_nominal_identities,
        foreign_same_application,
        foreign_data,
    )
    .unwrap();

    let target_applied = AppliedEnumVariantRef::checked(
        &enums,
        &applications,
        &nominal_identities,
        foreign_applied.application(),
        foreign_applied.declaration(),
    )
    .expect("same coordinates are revalidated in the target stores");
    assert_eq!(target_applied, foreign_applied);
    assert_eq!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            foreign_applied,
            0,
        )
        .expect("the target variant has field zero")
        .variant(),
        target_applied
    );
    assert!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            foreign_applied,
            1,
        )
        .is_none(),
        "a field index valid only in the foreign store must be rejected"
    );

    let foreign_wrong_owner = AppliedEnumVariantRef::checked(
        &foreign_enums,
        &foreign_applications,
        &foreign_nominal_identities,
        foreign_wrong_owner_application,
        foreign_data,
    )
    .unwrap();
    assert!(
        AppliedEnumVariantRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            foreign_wrong_owner.application(),
            foreign_wrong_owner.declaration(),
        )
        .is_none(),
        "the target application owner decides the relation"
    );
    assert!(
        AppliedEnumVariantFieldRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            foreign_wrong_owner,
            0,
        )
        .is_none(),
        "field construction must revalidate the applied variant owner"
    );

    let foreign_extra = EnumVariantRef::checked(&foreign_enums, foreign_choice, 2).unwrap();
    assert!(
        AppliedEnumVariantRef::checked(
            &enums,
            &applications,
            &nominal_identities,
            choice_application,
            foreign_extra,
        )
        .is_none(),
        "a variant index valid only in the foreign store must be rejected"
    );

    let mut invalid_applications = Arena::new();
    let invalid_application = invalid_applications.alloc(EnumApplication {
        template: nominal_identity("Missing", scoop_identity::SourceNominalKind::Enum, 0)
            .declaration_id(),
        arguments: Vec::new(),
        canonical_type: unit,
    });
    assert!(
        AppliedEnumVariantRef::checked(
            &enums,
            &invalid_applications,
            &nominal_identities,
            invalid_application,
            foreign_data,
        )
        .is_none(),
        "an application with an invalid target-store owner must be rejected"
    );
}
