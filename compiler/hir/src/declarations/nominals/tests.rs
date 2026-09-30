use super::*;

mod applications;
mod origins;

fn nominal_identity(
    name: &str,
    kind: scoop_identity::SourceNominalKind,
    parameters: usize,
) -> HirNominalIdentity {
    use scoop_identity::*;
    let source = SourceIdentity::single_file();
    let site = SourceDeclarationSite::new(
        source.cone(),
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::SourceScoped(source),
    )
    .unwrap();
    HirNominalIdentity::from_source_declaration(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        parameters as u32,
    ))
    .unwrap()
}

fn identities(structs: &Arena<StructDecl>, enums: &Arena<EnumDecl>) -> HirNominalIdentities {
    HirNominalIdentities::checked(
        structs,
        structs
            .iter()
            .map(|(_, value)| {
                nominal_identity(
                    &value.name,
                    scoop_identity::SourceNominalKind::Struct,
                    value.type_params.len(),
                )
            })
            .collect(),
        enums,
        enums
            .iter()
            .map(|(_, value)| {
                nominal_identity(
                    &value.name,
                    scoop_identity::SourceNominalKind::Enum,
                    value.type_params.len(),
                )
            })
            .collect(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
    )
    .unwrap()
}

fn enum_declaration(
    name: &str,
    self_application: EnumApplicationId,
    variants: Vec<Variant>,
) -> EnumDecl {
    EnumDecl {
        name: name.to_string(),
        owner: None,
        access: NominalAccess::public(),
        self_application,
        type_params: Vec::new(),
        gc_free_pointee_requirements: Vec::new(),
        no_gc: false,
        variants,
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),
    }
}

fn struct_declaration(
    name: &str,
    self_application: StructApplicationId,
    representation: StructRepresentation,
) -> StructDecl {
    StructDecl {
        name: name.to_string(),
        owner: None,
        access: NominalAccess::public(),
        self_application,
        type_params: Vec::new(),
        gc_free_pointee_requirements: Vec::new(),
        attributes: StructAttributes::default(),
        representation,
        constructors: Vec::new(),
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),
    }
}

#[test]
fn c_layout_accepts_only_canonical_long_values() {
    assert_eq!(
        HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(8)),
        Some(HirCLayoutValue::A8)
    );
    assert_eq!(
        HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(0)),
        Some(HirCLayoutValue::Natural)
    );
    assert_eq!(
        HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(3)),
        None
    );
    assert_eq!(
        HirCLayoutValue::from_integer(HirIntegerConstant::Signed32(8)),
        None
    );
    assert_eq!(
        HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(u64::MAX)),
        None
    );
}

#[test]
fn option_core_revalidates_refs_and_field_types_before_indexing() {
    let parameter = TypeParamId::from_raw(7);
    let mut types = Arena::new();
    let parameter_ty = types.alloc(Type::Param(parameter));
    let mut enums = Arena::new();
    let option = enums.alloc(enum_declaration(
        "Option",
        EnumApplicationId::from_raw(0.into()),
        vec![
            Variant {
                name: "Some".to_string(),
                style: VariantStyle::Positional,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: parameter_ty,
                }],
            },
            Variant {
                name: "None".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            },
        ],
    ));
    enums[option].type_params.push(TypeParamDecl {
        id: parameter,
        name: "T".to_string(),
        bounds: TypeParamBounds::Unconstrained,
        span: Span::new(0, 0),
    });
    let other = enums.alloc(enum_declaration(
        "Other",
        EnumApplicationId::from_raw(1.into()),
        vec![Variant {
            name: "None".to_string(),
            style: VariantStyle::Unit,
            fields: Vec::new(),
        }],
    ));
    let some = EnumVariantRef::checked(&enums, option, 0).unwrap();
    let some_payload = EnumVariantFieldRef::checked(&enums, some, 0).unwrap();
    let none = EnumVariantRef::checked(&enums, option, 1).unwrap();
    assert!(OptionCore::checked(&enums, &types, some_payload, none).is_some());

    let foreign_none = EnumVariantRef::checked(&enums, other, 0).unwrap();
    assert!(OptionCore::checked(&enums, &types, some_payload, foreign_none).is_none());

    let mut foreign_enums = Arena::new();
    let foreign_option = foreign_enums.alloc(enum_declaration(
        "Foreign",
        EnumApplicationId::from_raw(0.into()),
        vec![
            Variant {
                name: "Payload".to_string(),
                style: VariantStyle::Named,
                fields: vec![
                    Field {
                        name: "first".to_string(),
                        ty: parameter_ty,
                    },
                    Field {
                        name: "second".to_string(),
                        ty: parameter_ty,
                    },
                ],
            },
            Variant {
                name: "Empty".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            },
            Variant {
                name: "Extra".to_string(),
                style: VariantStyle::Named,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: parameter_ty,
                }],
            },
        ],
    ));
    let foreign_some = EnumVariantRef::checked(&foreign_enums, foreign_option, 0).unwrap();
    let foreign_payload = EnumVariantFieldRef::checked(&foreign_enums, foreign_some, 0).unwrap();
    let foreign_none = EnumVariantRef::checked(&foreign_enums, foreign_option, 1).unwrap();
    let from_foreign_coordinates =
        OptionCore::checked(&enums, &types, foreign_payload, foreign_none)
            .expect("foreign refs with valid target coordinates have no arena brand");
    assert_eq!(from_foreign_coordinates.some_payload(), some_payload);
    assert_eq!(from_foreign_coordinates.none(), none);

    let foreign_second_payload =
        EnumVariantFieldRef::checked(&foreign_enums, foreign_some, 1).unwrap();
    assert!(
        OptionCore::checked(&enums, &types, foreign_second_payload, foreign_none).is_none(),
        "a payload index valid only in the foreign store must be rejected"
    );
    let foreign_extra = EnumVariantRef::checked(&foreign_enums, foreign_option, 2).unwrap();
    let foreign_extra_payload =
        EnumVariantFieldRef::checked(&foreign_enums, foreign_extra, 0).unwrap();
    assert!(
        OptionCore::checked(&enums, &types, foreign_extra_payload, foreign_none).is_none(),
        "a variant index valid only in the foreign store must be rejected"
    );

    let empty_types = Arena::new();
    assert!(OptionCore::checked(&enums, &empty_types, some_payload, none).is_none());
    let mut wrong_types = Arena::new();
    wrong_types.alloc(Type::String);
    assert!(OptionCore::checked(&enums, &wrong_types, some_payload, none).is_none());

    let mut invalid_type_enums = enums.clone();
    invalid_type_enums[option].variants[0].fields[0].ty = TypeId::from_raw(99.into());
    assert!(
        OptionCore::checked(&invalid_type_enums, &types, some_payload, none).is_none(),
        "an out-of-bounds field type coordinate must not be indexed"
    );
}
