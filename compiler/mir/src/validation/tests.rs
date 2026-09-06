use super::*;

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

fn module_with_variants(variants: Vec<VariantDef>) -> (Module, EnumId) {
    let mut enums = Arena::new();
    let enum_id = enums.alloc(EnumDef {
        name: "Choice".to_string(),
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

#[test]
fn typed_variant_and_field_refs_are_checked_by_the_definition_store() {
    let (module, enum_id) = module_with_variants(vec![
        variant_def("Left", vec![Type::Integer(IntegerKind::SIGNED_32)]),
        variant_def("Right", Vec::new()),
    ]);
    let left = MirVariantRef::new(&module.enums, enum_id, 0).expect("Left exists");
    assert_eq!(left.enum_id(), enum_id);
    assert_eq!(left.variant_index(), 0);
    let field = MirVariantFieldRef::new(&module.enums, left, 0).expect("Left._0 exists");
    assert_eq!(field.variant(), left);
    assert_eq!(field.field_index(), 0);
    assert_eq!(
        field.definition(&module.enums).unwrap().ty,
        Type::Integer(IntegerKind::SIGNED_32)
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
fn safe_expression_constructors_fix_results_and_reject_a_wrong_enum() {
    let (mut module, enum_id) = module_with_variants(vec![variant_def(
        "Only",
        vec![Type::Integer(IntegerKind::SIGNED_32)],
    )]);
    let other = module.enums.alloc(EnumDef {
        name: "Other".to_string(),
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
        name: "Choice".to_string(),
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
        name: "Other".to_string(),
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
