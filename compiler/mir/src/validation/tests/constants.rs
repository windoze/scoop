use super::*;

fn encoded_global(module: &mut Module, ty: Type, payload: MirConstantImage) -> GlobalId {
    module.globals.alloc(Global {
        name: "constant".to_string(),
        symbol: "scoop.constant".to_string(),
        storage_owner: test_static_storage_owner("constant"),
        ty,
        mutable: false,
        storage: GlobalStorage::Local {
            thread_local: false,
            initial_state: MirStaticInitialState::EncodedStaticValue { payload },
        },
    })
}

fn global_payload_mut(module: &mut Module, global: GlobalId) -> &mut MirConstantImage {
    let GlobalStorage::Local {
        initial_state: MirStaticInitialState::EncodedStaticValue { payload },
        ..
    } = &mut module.globals[global].storage
    else {
        panic!("fixture global has an encoded local image")
    };
    payload
}

#[test]
fn string_global_validation_rejects_an_unknown_string_reference() {
    let (mut module, _) = module_with_variants(Vec::new());
    let unknown = StringConstId::from_raw(99.into());
    let global = encoded_global(&mut module, Type::String, MirConstantImage::String(unknown));

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global },
            kind: MirValidationErrorKind::InvalidConstantImage {
                path: Vec::new(),
                expected: Type::String,
                error: MirConstantImageError::InvalidStringReference { string: unknown },
            },
        })
    );
}

#[test]
fn struct_global_validation_rejects_an_unknown_struct_reference() {
    let (mut module, _) = module_with_variants(Vec::new());
    let unknown = StructId::from_raw(99.into());
    let global = encoded_global(
        &mut module,
        Type::Struct(unknown),
        MirConstantImage::Struct {
            struct_id: unknown,
            fields: Vec::new(),
        },
    );

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global },
            kind: MirValidationErrorKind::InvalidConstantImage {
                path: Vec::new(),
                expected: Type::Struct(unknown),
                error: MirConstantImageError::InvalidStructReference { struct_id: unknown },
            },
        })
    );
}

#[test]
fn struct_global_validation_rejects_an_inexact_field_arity() {
    let (mut module, _) = module_with_variants(Vec::new());
    let structure = module.structs.alloc(StructDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        name: "Pair".to_string(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![Field {
                name: "value".to_string(),
                ty: Type::Boolean,
            }],
        },
    });
    let global = encoded_global(
        &mut module,
        Type::Struct(structure),
        MirConstantImage::Struct {
            struct_id: structure,
            fields: Vec::new(),
        },
    );

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global },
            kind: MirValidationErrorKind::InvalidConstantImage {
                path: Vec::new(),
                expected: Type::Struct(structure),
                error: MirConstantImageError::StructArity {
                    struct_id: structure,
                    expected: 1,
                    actual: 0,
                },
            },
        })
    );
}

#[test]
fn enum_unit_global_validation_checks_exact_type_payload_and_reference() {
    let int = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Payload", vec![int]),
        variant_def("Empty", Vec::new()),
    ]);
    let payload = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let empty = MirVariantRef::new(&module.enums, enum_id, 1).unwrap();
    let global = encoded_global(
        &mut module,
        Type::Enum(enum_id, Vec::new()),
        MirConstantImage::EnumUnit { variant: empty },
    );
    assert_eq!(module.validate(), Ok(()));

    module.globals[global].ty = Type::Enum(enum_id, vec![Type::Boolean]);
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global: found },
            kind: MirValidationErrorKind::InvalidConstantImage {
                error: MirConstantImageError::TypeMismatch { image: "EnumUnit image" },
                ..
            },
        }) if found == global
    ));
    module.globals[global].ty = Type::Enum(enum_id, Vec::new());

    *global_payload_mut(&mut module, global) = MirConstantImage::EnumUnit { variant: payload };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global: found },
            kind: MirValidationErrorKind::InvalidConstantImage {
                error: MirConstantImageError::EnumUnitHasPayload { variant },
                ..
            },
        }) if found == global && variant == payload
    ));

    let other = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Other".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant_def("Empty", Vec::new())],
    });
    let other_empty = MirVariantRef::new(&module.enums, other, 0).unwrap();
    *global_payload_mut(&mut module, global) = MirConstantImage::EnumUnit {
        variant: other_empty,
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global: found },
            kind: MirValidationErrorKind::InvalidConstantImage {
                error: MirConstantImageError::TypeMismatch { image: "EnumUnit image" },
                ..
            },
        }) if found == global
    ));

    let mut foreign = Arena::new();
    let foreign_enum = foreign.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Foreign".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            variant_def("A", Vec::new()),
            variant_def("B", Vec::new()),
            variant_def("C", Vec::new()),
        ],
    });
    let foreign_variant = MirVariantRef::new(&foreign, foreign_enum, 2).unwrap();
    *global_payload_mut(&mut module, global) = MirConstantImage::EnumUnit {
        variant: foreign_variant,
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global: found },
            kind: MirValidationErrorKind::InvalidConstantImage {
                error: MirConstantImageError::InvalidVariantReference {
                    error: MirVariantRefError::VariantOutOfBounds { variant: 2, .. },
                },
                ..
            },
        }) if found == global
    ));
}

#[test]
fn nested_global_constant_validation_tracks_the_exact_field_path() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def("Empty", Vec::new())]);
    let empty = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let inner = module.structs.alloc(StructDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        name: "Inner".to_string(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![Field {
                name: "choice".to_string(),
                ty: Type::Enum(enum_id, Vec::new()),
            }],
        },
    });
    let outer = module.structs.alloc(StructDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        name: "Outer".to_string(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![Field {
                name: "inner".to_string(),
                ty: Type::Struct(inner),
            }],
        },
    });
    let global = encoded_global(
        &mut module,
        Type::Struct(outer),
        MirConstantImage::Struct {
            struct_id: outer,
            fields: vec![MirConstantImage::Struct {
                struct_id: inner,
                fields: vec![MirConstantImage::EnumUnit { variant: empty }],
            }],
        },
    );
    assert_eq!(module.validate(), Ok(()));

    let MirConstantImage::Struct { fields, .. } = global_payload_mut(&mut module, global) else {
        unreachable!()
    };
    let MirConstantImage::Struct {
        fields: inner_fields,
        ..
    } = &mut fields[0]
    else {
        unreachable!()
    };
    inner_fields[0] = MirConstantImage::Boolean(false);
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::Global { global: found },
            kind: MirValidationErrorKind::InvalidConstantImage {
                path,
                expected: Type::Enum(found_enum, arguments),
                error: MirConstantImageError::TypeMismatch { image: "Boolean image" },
            },
        }) if found == global && path == vec![0, 0] && found_enum == enum_id && arguments.is_empty()
    ));
}
