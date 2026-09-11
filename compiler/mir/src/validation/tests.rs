use super::*;

mod boxed_values;
mod callbacks;
mod closure_environments;
mod constants;
mod coroutines;
mod function_bridges;
mod metadata;

fn nominal_link_stem() -> NominalLinkStem {
    NominalLinkStem::from_session_local_encoding("$mir-validation-test$nominal".to_string())
}

fn variant_def(name: &str, fields: Vec<Type>) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: fields
            .into_iter()
            .enumerate()
            .map(|(index, ty)| Field {
                name: format!("_{index}"),
                ty,
            })
            .collect(),
    }
}

fn test_exact_type(
    ty: &Type,
) -> scoop_identity::CborIdentityRecord<
    scoop_identity::PersistentExactTypeId,
    scoop_identity::ExactTypeKey,
> {
    if matches!(ty, Type::Unit) {
        return scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ),
        )
        .unwrap();
    }
    if matches!(ty, Type::Any) {
        return scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Any
                    .identity_record()
                    .id(),
            ),
        )
        .unwrap();
    }
    let owner_tag = match ty {
        Type::Integer(_) => 2,
        Type::Boolean => 3,
        Type::String => 4,
        Type::Class(id) => 32 + id.into_raw().into_u32(),
        _ => 255,
    };
    let site = scoop_identity::SourceDeclarationSite::new(
        scoop_identity::ConeIdentity::SINGLE_FILE,
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::ConeWide,
    )
    .unwrap();
    let name = format!("TestType{owner_tag}");
    let declaration = scoop_identity::SourceDeclarationKey::nominal(
        site,
        scoop_identity::CanonicalIdentifier::new(&name).unwrap(),
        scoop_identity::SourceNominalKind::Struct,
        0,
    );
    let owner = scoop_identity::PersistentTypeId::from_source_declaration(&declaration).unwrap();
    scoop_identity::CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(owner))
        .unwrap()
}

fn register_test_exact_type(module: &mut Module, ty: &Type) {
    if let Some(found) = module.meta.source_exact_types.get(ty) {
        assert_eq!(found.identity_record().id(), test_exact_type(ty).id());
        return;
    }
    let mut entries = module
        .meta
        .source_exact_types
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceExactTypeIdentity::checked(ty.clone(), test_exact_type(ty), None).unwrap());
    module.meta.source_exact_types = SourceExactTypeIdentities::checked(entries).unwrap();
}

fn test_step_identity(ty: &Type) -> CoroutineStepIdentity {
    CoroutineStepIdentity::new(&test_exact_type(ty), None).unwrap()
}

fn test_slot_identity(ty: &Type) -> CoroutineSlotIdentity {
    CoroutineSlotIdentity::new(&test_exact_type(ty), None).unwrap()
}

fn test_continuation_shell_identity(ty: &Type) -> ContinuationShellIdentity {
    ContinuationShellIdentity::new(&test_exact_type(ty), None).unwrap()
}

fn test_coroutine_start_identity(ty: &Type) -> CoroutineStartIdentity {
    CoroutineStartIdentity::new(&test_exact_type(ty), None).unwrap()
}

fn module_with_variants(variants: Vec<VariantDef>) -> (Module, EnumId) {
    let mut enums = Arena::new();
    let enum_id = enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Choice".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants,
    });
    let mut functions = Arena::new();
    let entry = functions.alloc(Function {
        gc_effect: GcEffect::NoGc,
        name: "main".to_string(),
        symbol: ENTRY_SYMBOL.to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body::unreachable(Arena::new()),
    });
    (
        Module {
            functions,
            extern_functions: Arena::new(),
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            callback_bridges: Arena::new(),
            foreign_callback_adapters: Arena::new(),
            foreign_callback_families: Arena::new(),
            foreign_callback_bridges: Arena::new(),
            function_types: Arena::new(),
            closure_classes: Arena::new(),
            closure_invoke_functions: Arena::new(),
            top_level: vec![entry],
            strings: Arena::new(),
            structs: Arena::new(),
            enums,
            classes: Arena::new(),
            interfaces: Arena::new(),
            option_core: Vec::new(),
            entry,
            meta: MirMeta::default(),
        },
        enum_id,
    )
}

fn enum_local(locals: &mut Arena<Local>, name: &str, enum_id: EnumId) -> LocalId {
    locals.alloc(Local {
        name: name.to_string(),
        ty: Type::Enum(enum_id, Vec::new()),
        mutable: false,
    })
}

fn guarded_body(
    module: &Module,
    test_local: LocalId,
    project_local: LocalId,
    locals: Arena<Local>,
    test_variant: MirVariantRef,
    project_field: MirVariantFieldRef,
    project_on_true: bool,
) -> (Body, BlockId) {
    let enum_id = test_variant.enum_id();
    let operand_ty = Type::Enum(enum_id, Vec::new());
    let test = Expr::variant_test(
        &module.enums,
        Expr::local(test_local, operand_ty.clone()),
        test_variant,
    )
    .expect("fixture uses a matching checked variant");
    let project = Expr::variant_payload_project(
        &module.enums,
        Expr::local(project_local, operand_ty),
        project_field,
    )
    .expect("fixture uses a matching checked field");
    let mut blocks = Arena::new();
    let projected = blocks.alloc(BasicBlock {
        name: "projected".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Return {
            value: Some(project),
        },
        unwind: None,
    });
    let other = blocks.alloc(BasicBlock {
        name: "other".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Unreachable,
        unwind: None,
    });
    let (then_block, else_block) = if project_on_true {
        (projected, other)
    } else {
        (other, projected)
    };
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Branch {
            cond: test,
            then_block,
            else_block,
        },
        unwind: None,
    });
    (
        Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
        projected,
    )
}

fn return_value_mut(module: &mut Module, block: BlockId) -> &mut Expr {
    let Terminator::Return { value: Some(value) } =
        &mut module.functions[module.entry].body.blocks[block].terminator
    else {
        panic!("fixture block returns a value")
    };
    value
}

fn module_with_declared_struct(fields: Vec<Type>) -> (Module, StructId) {
    let (mut module, _) = module_with_variants(Vec::new());
    let struct_id = module.structs.alloc(StructDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        name: "Record".to_string(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, ty)| Field {
                    name: format!("f{index}"),
                    ty,
                })
                .collect(),
        },
    });
    (module, struct_id)
}

fn set_return_expression(module: &mut Module, expression: Expr) {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Return {
            value: Some(expression),
        },
        unwind: None,
    });
    module.functions[module.entry].body = Body {
        locals: Arena::new(),
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    };
}

#[test]
fn loop_header_poll_target_must_belong_to_its_body() {
    let (mut module, _) = module_with_variants(Vec::new());
    let invalid = la_arena::Idx::from_raw(1.into());
    module.functions[module.entry].body.loop_header_polls =
        vec![LoopHeaderPollTarget::new(invalid)];

    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { block, .. },
            kind: MirValidationErrorKind::InvalidLoopHeaderPollTarget,
        }) if block == invalid
    ));
}

#[test]
fn loop_header_poll_target_must_be_unique_within_its_body() {
    let (mut module, _) = module_with_variants(Vec::new());
    let entry = module.functions[module.entry].body.entry;
    module.functions[module.entry].body.loop_header_polls = vec![
        LoopHeaderPollTarget::new(entry),
        LoopHeaderPollTarget::new(entry),
    ];

    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::FunctionBlock { block, .. },
            kind: MirValidationErrorKind::DuplicateLoopHeaderPollTarget,
        }) if block == entry
    ));
}

#[test]
fn loop_header_poll_target_may_be_detached_and_noncyclic() {
    let (mut module, _) = module_with_variants(Vec::new());
    let detached = module.functions[module.entry]
        .body
        .blocks
        .alloc(BasicBlock {
            name: "detached".to_string(),
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
            unwind: None,
        });
    module.functions[module.entry].body.loop_header_polls =
        vec![LoopHeaderPollTarget::new(detached)];

    assert_eq!(module.validate(), Ok(()));
}

#[test]
fn raw_struct_construction_validation_checks_identity_arity_and_field_types() {
    let int = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, struct_id) = module_with_declared_struct(vec![int.clone(), Type::Boolean]);
    set_return_expression(
        &mut module,
        Expr::new(
            Type::Struct(struct_id),
            ExprKind::StructConstruct {
                struct_id,
                fields: vec![
                    Expr::new(int.clone(), ExprKind::UnitLiteral),
                    Expr::new(Type::Boolean, ExprKind::BoolLiteral(true)),
                ],
            },
        ),
    );
    assert_eq!(module.validate(), Ok(()));

    let entry = module.functions[module.entry].body.entry;
    return_value_mut(&mut module, entry).ty = Type::Boolean;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::RawStructConstructResultType {
                struct_id: found,
                actual: Type::Boolean,
            },
            ..
        }) if found == struct_id
    ));

    {
        let expression = return_value_mut(&mut module, entry);
        expression.ty = Type::Struct(struct_id);
        let ExprKind::StructConstruct { fields, .. } = &mut expression.kind else {
            unreachable!()
        };
        fields.pop();
    }
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::RawStructConstructArity {
                expected: 2,
                actual: 1,
                ..
            },
            ..
        })
    ));

    {
        let expression = return_value_mut(&mut module, entry);
        let ExprKind::StructConstruct { fields, .. } = &mut expression.kind else {
            unreachable!()
        };
        fields.push(Expr::new(Type::Boolean, ExprKind::BoolLiteral(false)));
        fields[0].ty = Type::Boolean;
    }
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::RawStructConstructFieldType {
                field: 0,
                expected: Type::Integer(IntegerKind::SIGNED_32),
                actual: Type::Boolean,
                ..
            },
            ..
        })
    ));
}

#[test]
fn raw_struct_construction_validation_rejects_unknown_and_intrinsic_targets() {
    let (mut module, struct_id) = module_with_declared_struct(Vec::new());
    let unknown = StructId::from_raw(99.into());
    set_return_expression(
        &mut module,
        Expr::new(
            Type::Struct(unknown),
            ExprKind::StructConstruct {
                struct_id: unknown,
                fields: Vec::new(),
            },
        ),
    );
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidStructReference { struct_id: found },
            ..
        }) if found == unknown
    ));

    module.structs[struct_id].representation =
        StructRepresentation::Intrinsic(IntrinsicTypeRepresentation::Boolean);
    set_return_expression(
        &mut module,
        Expr::new(
            Type::Struct(struct_id),
            ExprKind::StructConstruct {
                struct_id,
                fields: Vec::new(),
            },
        ),
    );
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::RawStructConstructRequiresDeclared {
                struct_id: found
            },
            ..
        }) if found == struct_id
    ));
}

#[test]
fn typed_variant_and_field_refs_are_checked_by_the_definition_store() {
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Completed", vec![Type::Integer(IntegerKind::SIGNED_32)]),
        variant_def("Suspended", Vec::new()),
    ]);
    let left = MirVariantRef::new(&module.enums, enum_id, 0).expect("Left exists");
    assert_eq!(left.enum_id(), enum_id);
    assert_eq!(left.variant_index(), 0);
    let field = MirVariantFieldRef::new(&module.enums, left, 0).expect("Left._0 exists");
    let right = MirVariantRef::new(&module.enums, enum_id, 1).expect("Right exists");
    assert_eq!(field.variant(), left);
    assert_eq!(field.field_index(), 0);
    assert_eq!(
        field.definition(&module.enums).unwrap().ty,
        Type::Integer(IntegerKind::SIGNED_32)
    );
    let payload_ty = Type::Integer(IntegerKind::SIGNED_32);
    let step_identity = test_step_identity(&payload_ty);
    let step = CoroutineStep::checked(
        &module.enums,
        field,
        right,
        payload_ty.clone(),
        step_identity.clone(),
    )
    .expect("valid CoroutineStep shape");
    assert_eq!(step.completed(), left);
    assert_eq!(step.suspended(), right);
    module.enums[enum_id].variants[0].name = "Done".to_string();
    assert!(
        CoroutineStep::checked(
            &module.enums,
            field,
            right,
            payload_ty.clone(),
            step_identity.clone(),
        )
        .is_none(),
        "CoroutineStep roles have fixed generated names"
    );
    module.enums[enum_id].variants[0].name = "Completed".to_string();
    assert!(
        CoroutineStep::checked(
            &module.enums,
            field,
            right,
            Type::Boolean,
            step_identity.clone(),
        )
        .is_none()
    );
    module.enums[enum_id]
        .variants
        .push(variant_def("Unexpected", Vec::new()));
    assert!(
        CoroutineStep::checked(
            &module.enums,
            field,
            right,
            payload_ty.clone(),
            step_identity.clone(),
        )
        .is_none(),
        "CoroutineStep metadata requires exactly two variants"
    );
    module.enums[enum_id].variants.pop();
    module.enums[enum_id].type_arguments.push(Type::Boolean);
    assert!(
        CoroutineStep::checked(
            &module.enums,
            field,
            right,
            payload_ty.clone(),
            step_identity.clone(),
        )
        .is_none(),
        "CoroutineStep metadata is already concrete"
    );
    module.enums[enum_id].type_arguments.clear();

    let option_enum = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Option".to_string(),
        type_arguments: vec![payload_ty.clone()],
        gc_free: true,
        variants: vec![
            variant_def("Some", vec![payload_ty.clone()]),
            variant_def("None", Vec::new()),
        ],
    });
    let some = MirVariantRef::new(&module.enums, option_enum, 0).unwrap();
    let some_payload = MirVariantFieldRef::new(&module.enums, some, 0).unwrap();
    let none = MirVariantRef::new(&module.enums, option_enum, 1).unwrap();
    let option =
        OptionCore::checked(&module.enums, some_payload, none).expect("valid Option shape");
    assert_eq!(option.some(), some);
    assert_eq!(option.some_payload(), some_payload);
    assert_eq!(option.none(), none);
    assert!(OptionCore::checked(&module.enums, some_payload, some).is_none());
    module.enums[option_enum]
        .variants
        .push(variant_def("Unexpected", Vec::new()));
    assert!(
        OptionCore::checked(&module.enums, some_payload, none).is_none(),
        "Option metadata requires exactly two variants"
    );
    module.enums[option_enum].variants.pop();
    module.enums[option_enum].type_arguments[0] = Type::Boolean;
    assert!(OptionCore::checked(&module.enums, some_payload, none).is_none());
    module.enums[option_enum].type_arguments[0] = payload_ty.clone();

    let foreign_enum = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Foreign".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant_def("None", Vec::new())],
    });
    let foreign_none = MirVariantRef::new(&module.enums, foreign_enum, 0).unwrap();
    assert!(OptionCore::checked(&module.enums, some_payload, foreign_none).is_none());
    assert!(
        CoroutineStep::checked(
            &module.enums,
            field,
            foreign_none,
            payload_ty.clone(),
            step_identity,
        )
        .is_none()
    );

    let slot_enum = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "CoroutineSlot".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            variant_def("Empty", Vec::new()),
            variant_def("Value", vec![payload_ty.clone()]),
        ],
    });
    let empty = MirVariantRef::new(&module.enums, slot_enum, 0).unwrap();
    let value = MirVariantRef::new(&module.enums, slot_enum, 1).unwrap();
    let value_field = MirVariantFieldRef::new(&module.enums, value, 0).unwrap();
    let slot = CoroutineSlot::checked(
        &module.enums,
        value_field,
        empty,
        payload_ty.clone(),
        test_slot_identity(&payload_ty),
    )
    .expect("valid CoroutineSlot shape");
    assert_eq!(slot.value_variant(), value);
    assert_eq!(slot.empty(), empty);
    module.enums[slot_enum].variants[1].name = "Present".to_string();
    assert!(
        CoroutineSlot::checked(
            &module.enums,
            value_field,
            empty,
            payload_ty.clone(),
            test_slot_identity(&payload_ty),
        )
        .is_none(),
        "CoroutineSlot roles have fixed generated names"
    );
    module.enums[slot_enum].variants[1].name = "Value".to_string();
    assert!(
        CoroutineSlot::checked(
            &module.enums,
            value_field,
            empty,
            Type::Boolean,
            test_slot_identity(&payload_ty),
        )
        .is_none()
    );
    assert!(
        CoroutineSlot::checked(
            &module.enums,
            value_field,
            foreign_none,
            payload_ty,
            test_slot_identity(&Type::Integer(IntegerKind::SIGNED_32)),
        )
        .is_none()
    );

    assert!(matches!(
        MirVariantRef::new(&module.enums, enum_id, 2),
        Err(MirVariantRefError::VariantOutOfBounds {
            variant: 2,
            variant_count: 2,
            ..
        })
    ));
    let unknown = EnumId::from_raw(99.into());
    assert!(matches!(
        MirVariantRef::new(&module.enums, unknown, 0),
        Err(MirVariantRefError::UnknownEnum { enum_id: found }) if found == unknown
    ));
    assert!(matches!(
        MirVariantFieldRef::new(&module.enums, left, 1),
        Err(MirVariantFieldRefError::FieldOutOfBounds {
            field: 1,
            field_count: 1,
            ..
        })
    ));
}

#[test]
fn variant_construction_validation_checks_ref_result_arity_and_field_types() {
    let int = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![variant_def("Value", vec![int.clone()])]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let construct = |ty, fields| Expr::new(ty, ExprKind::VariantConstruct { variant, fields });
    set_return_expression(
        &mut module,
        construct(
            Type::Enum(enum_id, Vec::new()),
            vec![Expr::new(int.clone(), ExprKind::UnitLiteral)],
        ),
    );
    assert_eq!(module.validate(), Ok(()));
    let entry = module.functions[module.entry].body.entry;

    return_value_mut(&mut module, entry).ty = Type::Boolean;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantConstructResultType {
                actual: Type::Boolean,
                ..
            },
            ..
        })
    ));

    let expression = return_value_mut(&mut module, entry);
    expression.ty = Type::Enum(enum_id, vec![Type::Boolean]);
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantConstructResultType {
                actual: Type::Enum(found, arguments),
                ..
            },
            ..
        }) if found == enum_id && arguments == vec![Type::Boolean]
    ));

    let expression = return_value_mut(&mut module, entry);
    expression.ty = Type::Enum(enum_id, Vec::new());
    let ExprKind::VariantConstruct { fields, .. } = &mut expression.kind else {
        unreachable!()
    };
    fields.clear();
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantConstructArity {
                expected: 1,
                actual: 0,
                ..
            },
            ..
        })
    ));

    let ExprKind::VariantConstruct { fields, .. } = &mut return_value_mut(&mut module, entry).kind
    else {
        unreachable!()
    };
    fields.push(Expr::new(Type::Boolean, ExprKind::BoolLiteral(false)));
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantConstructFieldType {
                expected: Type::Integer(IntegerKind::SIGNED_32),
                actual: Type::Boolean,
                ..
            },
            ..
        })
    ));

    let mut foreign = Arena::new();
    let foreign_enum = foreign.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Choice".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            variant_def("Value", vec![int]),
            variant_def("Other", Vec::new()),
        ],
    });
    let invalid = MirVariantRef::new(&foreign, foreign_enum, 1).unwrap();
    let expression = return_value_mut(&mut module, entry);
    expression.kind = ExprKind::VariantConstruct {
        variant: invalid,
        fields: Vec::new(),
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidVariantReference {
                operation: MirVariantOperation::Construct,
                error: MirVariantRefError::VariantOutOfBounds { variant: 1, .. },
            },
            ..
        })
    ));
}

#[test]
fn safe_expression_constructors_fix_results_and_reject_a_wrong_enum() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let other = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Other".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant_def("Only", Vec::new())],
    });
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
    let operand = Expr::new(Type::Enum(enum_id, Vec::new()), ExprKind::UnitLiteral);
    let test = Expr::variant_test(&module.enums, operand.clone(), variant).unwrap();
    let project = Expr::variant_payload_project(&module.enums, operand, field).unwrap();
    assert_eq!(test.ty, Type::Boolean);
    assert_eq!(project.ty, Type::Integer(IntegerKind::SIGNED_32));

    module.enums[enum_id].type_arguments = vec![Type::Boolean];
    let wrong_arguments = Expr::new(Type::Enum(enum_id, Vec::new()), ExprKind::UnitLiteral);
    assert!(matches!(
        Expr::variant_test(&module.enums, wrong_arguments, variant),
        Err(MirVariantExprError::OperandTypeArgumentsMismatch {
            enum_id: found,
            expected,
            actual,
        }) if found == enum_id && expected == vec![Type::Boolean] && actual.is_empty()
    ));
    module.enums[enum_id].type_arguments.clear();

    let wrong = Expr::new(Type::Enum(other, Vec::new()), ExprKind::UnitLiteral);
    assert!(matches!(
        Expr::variant_test(&module.enums, wrong, variant),
        Err(MirVariantExprError::OperandEnumMismatch { expected, actual })
            if expected == enum_id && actual == other
    ));
}

#[test]
fn dump_and_visitors_cover_representation_independent_variant_nodes() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (body, _) = guarded_body(&module, value, value, locals, variant, field, true);
    module.functions[module.entry].return_ty = Type::Integer(IntegerKind::SIGNED_32);
    module.functions[module.entry].body = body;

    let text = dump(&module);
    assert!(text.contains("VariantTest Choice v0"), "{text}");
    assert!(
        text.contains("VariantPayloadProject Choice v0 f0"),
        "{text}"
    );

    let test = match &module.functions[module.entry].body.blocks
        [module.functions[module.entry].body.entry]
        .terminator
    {
        Terminator::Branch { cond, .. } => cond,
        _ => unreachable!(),
    };
    let mut visited = 0;
    visit_expr(test, &mut |_| visited += 1);
    assert_eq!(visited, 2, "the test and its operand are both visited");
    let mut test = test.clone();
    let mut visited_mut = 0;
    visit_expr_mut(&mut test, &mut |_| visited_mut += 1);
    assert_eq!(visited_mut, 2);
}

#[test]
fn validation_accepts_only_the_matching_true_edge_for_the_same_stable_value() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (body, _) = guarded_body(&module, value, value, locals, variant, field, true);
    module.functions[module.entry].return_ty = Type::Integer(IntegerKind::SIGNED_32);
    module.functions[module.entry].body = body;
    assert_eq!(module.validate(), Ok(()));

    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (body, _) = guarded_body(&module, value, value, locals, variant, field, false);
    module.functions[module.entry].body = body;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadNotDominated { field: found },
            ..
        }) if found == field
    ));

    let mut locals = Arena::new();
    let tested = enum_local(&mut locals, "tested", enum_id);
    let projected = enum_local(&mut locals, "projected", enum_id);
    let (body, _) = guarded_body(&module, tested, projected, locals, variant, field, true);
    module.functions[module.entry].body = body;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadNotDominated { .. },
            ..
        })
    ));
}

#[test]
fn validation_rejects_projection_without_a_matching_test_edge() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (mut body, projected) = guarded_body(&module, value, value, locals, variant, field, true);
    body.blocks[body.entry].terminator = Terminator::Goto(projected);
    module.functions[module.entry].return_ty = Type::Integer(IntegerKind::SIGNED_32);
    module.functions[module.entry].body = body;

    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadNotDominated { field: found },
            ..
        }) if found == field
    ));
}

#[test]
fn mutable_or_assigned_locals_do_not_supply_stable_variant_identity() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();

    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    locals[value].mutable = true;
    let (body, _) = guarded_body(&module, value, value, locals, variant, field, true);
    module.functions[module.entry].return_ty = Type::Integer(IntegerKind::SIGNED_32);
    module.functions[module.entry].body = body;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadNotDominated { .. },
            ..
        })
    ));

    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (mut body, _) = guarded_body(&module, value, value, locals, variant, field, true);
    body.blocks[body.entry].statements.push(Statement {
        kind: StatementKind::Assign {
            local: value,
            value: Expr::local(value, Type::Enum(enum_id, Vec::new())),
        },
        span: Span::new(0, 0),
    });
    module.functions[module.entry].body = body;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadNotDominated { .. },
            ..
        })
    ));
}

#[test]
fn validation_rejects_wrong_variant_field_and_result_contracts() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let field = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", enum_id);
    let (body, projected) = guarded_body(&module, value, value, locals, variant, field, true);
    module.functions[module.entry].return_ty = Type::Integer(IntegerKind::SIGNED_32);
    module.functions[module.entry].body = body;

    return_value_mut(&mut module, projected).ty = Type::Boolean;
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantPayloadResultType {
                expected: Type::Integer(IntegerKind::SIGNED_32),
                actual: Type::Boolean,
                ..
            },
            ..
        })
    ));

    let mut foreign = Arena::new();
    let foreign_enum = foreign.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Choice".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant_def(
            "Only",
            vec![
                Type::Integer(IntegerKind::SIGNED_32),
                Type::Integer(IntegerKind::SIGNED_32),
            ],
        )],
    });
    let foreign_variant = MirVariantRef::new(&foreign, foreign_enum, 0).unwrap();
    let foreign_field = MirVariantFieldRef::new(&foreign, foreign_variant, 1).unwrap();
    let invalid = return_value_mut(&mut module, projected);
    invalid.ty = Type::Integer(IntegerKind::SIGNED_32);
    invalid.kind = ExprKind::VariantPayloadProject {
        operand: Box::new(Expr::local(value, Type::Enum(enum_id, Vec::new()))),
        field: foreign_field,
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidVariantFieldReference {
                error: MirVariantFieldRefError::FieldOutOfBounds { field: 1, .. }
            },
            ..
        })
    ));

    return_value_mut(&mut module, projected).kind = ExprKind::VariantPayloadProject {
        operand: Box::new(Expr::local(value, Type::Enum(enum_id, Vec::new()))),
        field,
    };
    let foreign_variant_one = {
        foreign[foreign_enum]
            .variants
            .push(variant_def("Second", Vec::new()));
        MirVariantRef::new(&foreign, foreign_enum, 1).unwrap()
    };
    let entry = module.functions[module.entry].body.entry;
    let Terminator::Branch { cond, .. } =
        &mut module.functions[module.entry].body.blocks[entry].terminator
    else {
        unreachable!()
    };
    cond.kind = ExprKind::VariantTest {
        operand: Box::new(Expr::local(value, Type::Enum(enum_id, Vec::new()))),
        variant: foreign_variant_one,
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::InvalidVariantReference {
                error: MirVariantRefError::VariantOutOfBounds { variant: 1, .. },
                ..
            },
            ..
        })
    ));
}

#[test]
fn validation_rejects_wrong_enum_and_test_result_type() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def("Only", Vec::new())]);
    let other = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Other".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![variant_def("Only", Vec::new())],
    });
    let variant = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let mut locals = Arena::new();
    let value = enum_local(&mut locals, "value", other);
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Expr::new(
                Type::Boolean,
                ExprKind::VariantTest {
                    operand: Box::new(Expr::local(value, Type::Enum(other, Vec::new()))),
                    variant,
                },
            )),
        },
        unwind: None,
    });
    module.functions[module.entry].return_ty = Type::Boolean;
    module.functions[module.entry].body = Body {
        locals,
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    };
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantOperandEnumMismatch {
                expected,
                actual,
                ..
            },
            ..
        }) if expected == enum_id && actual == other
    ));

    let Terminator::Return { value: Some(test) } =
        &mut module.functions[module.entry].body.blocks[entry].terminator
    else {
        unreachable!()
    };
    test.ty = Type::Integer(IntegerKind::SIGNED_32);
    let ExprKind::VariantTest { operand, .. } = &mut test.kind else {
        unreachable!()
    };
    operand.ty = Type::Enum(enum_id, Vec::new());
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            kind: MirValidationErrorKind::VariantTestResultType {
                actual: Type::Integer(IntegerKind::SIGNED_32)
            },
            ..
        })
    ));
}
