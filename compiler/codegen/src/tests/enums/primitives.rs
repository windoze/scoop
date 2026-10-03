use super::*;

fn append_variant_projection_function(
    module: &mut Module,
    symbol: &str,
    operand_ty: LirType,
    field: scoop_lir::LirVariantFieldRef,
    result_ty: LirType,
) -> usize {
    let mut temps = Arena::default();
    let tested = temps.alloc(Temp { ty: LirType::I1 });
    let projected = temps.alloc(Temp {
        ty: result_ty.clone(),
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    let matched = blocks.alloc(BasicBlock {
        name: "matched".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    let missed = blocks.alloc(BasicBlock {
        name: "missed".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::VariantTest {
            out: tested,
            operand: Value::Param(0),
            variant: field.variant(),
        }],
        terminator: Terminator::CondBr {
            cond: Value::Temp(tested),
            then_block: matched,
            else_block: missed,
        },
    };
    blocks[matched] = BasicBlock {
        name: "matched".to_string(),
        instructions: vec![Instruction::VariantPayloadProject {
            out: projected,
            operand: Value::Param(0),
            field,
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(projected)),
        },
    };
    let signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![operand_ty],
        result_ty.clone(),
    );
    let fallback = match result_ty {
        LirType::Ptr(kind) => Value::NullPointer(kind),
        LirType::I1 => Value::BoolConst(false),
        LirType::I64 => signed64(0),
        _ => signed64(0),
    };
    blocks[missed] = BasicBlock {
        name: "missed".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(fallback),
        },
    };
    let index = module.functions.len();
    module.functions.push(Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    });
    index
}

fn append_variant_test_function(
    module: &mut Module,
    symbol: &str,
    operand_ty: LirType,
    variant: scoop_lir::LirVariantRef,
    result_ty: LirType,
) -> usize {
    let signature = scoop_signature(
        &module.structs,
        &module.enums,
        vec![operand_ty],
        LirType::I1,
    );
    let mut temps = Arena::default();
    let tested = temps.alloc(Temp { ty: result_ty });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::VariantTest {
            out: tested,
            operand: Value::Param(0),
            variant,
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(tested)),
        },
    });
    let index = module.functions.len();
    module.functions.push(Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    });
    index
}

#[test]
fn variant_primitives_emit_tagged_test_and_dominated_payload_projection() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let variant = module.enums.variant_ref(shape, 2).expect("Rect variant");
    let field = module
        .enums
        .variant_field_ref(variant, 1)
        .expect("Rect managed field");
    append_variant_projection_function(
        &mut module,
        "scoop.variant.tagged",
        LirType::Enum(shape),
        field,
        MANAGED_PTR,
    );

    let dump = scoop_lir::dump(&module);
    assert!(dump.contains("variant_test e0 v2 param0 : i1"), "{dump}");
    assert!(
        dump.contains("variant_payload_project e0 v2 f1 param0 : ptr<managed>"),
        "{dump}"
    );
    let ir = ir_of(&module);
    assert!(
        ir.contains("icmp eq i64") && ir.contains("i64 2"),
        "tagged variant test must compare the selected discriminant:\n{ir}"
    );
    assert!(
        ir.contains("variant_payload_field_ptr") && ir.contains("i64 24"),
        "tagged payload projection must use its checked layout offset:\n{ir}"
    );
    assert!(
        ir.contains("ptr addrspace(1)"),
        "managed payload projection must retain AS1 provenance:\n{ir}"
    );
}

#[test]
fn variant_primitives_emit_managed_raw_and_code_niche_provenance() {
    for kind in [
        scoop_lir::NichePointerKind::Managed,
        scoop_lir::NichePointerKind::Raw,
        scoop_lir::NichePointerKind::Code,
    ] {
        let scan = if kind == scoop_lir::NichePointerKind::Managed {
            RefScan::References(vec![0])
        } else {
            RefScan::None
        };
        let mut module = enum_module_with(kind, scan, LirType::I64);
        let option = module.enums.iter().nth(1).expect("niche enum").0;
        let payload = module
            .enums
            .variant_ref(option, 1)
            .expect("niche payload variant");
        let unit = module
            .enums
            .variant_ref(option, 0)
            .expect("niche unit variant");
        let field = module
            .enums
            .variant_field_ref(payload, 0)
            .expect("niche carrier field");
        let projection = append_variant_projection_function(
            &mut module,
            &format!("scoop.variant.niche.{}", kind.pointer_kind().dump()),
            LirType::Enum(option),
            field,
            LirType::Ptr(kind.pointer_kind()),
        );
        append_variant_test_function(
            &mut module,
            &format!("scoop.variant.unit.{}", kind.pointer_kind().dump()),
            LirType::Enum(option),
            unit,
            LirType::I1,
        );

        let ir = ir_of(&module);
        assert!(
            ir.contains("icmp ne") && ir.contains("icmp eq"),
            "niche payload/unit tests must be non-null/null comparisons:\n{ir}"
        );
        if kind == scoop_lir::NichePointerKind::Managed {
            assert!(
                ir.contains(&format!(
                    "define ptr addrspace(1) {}",
                    llvm_function_symbol(&module.functions[projection])
                )),
                "managed niche projection lost AS1 provenance:\n{ir}"
            );
        } else {
            assert!(
                ir.contains(&format!(
                    "define ptr {}",
                    llvm_function_symbol(&module.functions[projection])
                )),
                "raw/code niche projection lost its AS0 carrier:\n{ir}"
            );
        }
    }
}

#[test]
fn variant_test_rejects_wrong_enum_and_non_boolean_result() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let option = module.enums.iter().nth(1).expect("niche enum").0;
    let variant = module.enums.variant_ref(shape, 0).expect("Dot variant");
    append_variant_test_function(
        &mut module,
        "scoop.variant.wrong_enum",
        LirType::Enum(option),
        variant,
        LirType::I1,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_test")
            && error.0.contains("expects enum0")
            && error.0.contains("got enum1"),
        "unexpected error: {error}"
    );

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let variant = module.enums.variant_ref(shape, 0).expect("Dot variant");
    append_variant_test_function(
        &mut module,
        "scoop.variant.non_bool",
        LirType::Enum(shape),
        variant,
        LirType::I64,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_test") && error.0.contains("expected i1"),
        "unexpected error: {error}"
    );
}

#[test]
fn variant_references_are_revalidated_against_the_complete_module() {
    let mut foreign = scoop_lir::EnumDefs::default();
    foreign.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "padding",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "padding".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![EnumVariantRepr {
                fields: Vec::new(),
                slot_offset: 8,
                slot_size: 0,
                slot_align: 1,
                gc_free: true,
            }],
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    });
    let foreign_option = foreign.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "foreign",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "foreign".to_string(),
        repr: EnumRepr::Tagged {
            variants: (0..3)
                .map(|_| EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                })
                .collect(),
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    });
    let invalid = foreign
        .variant_ref(foreign_option, 2)
        .expect("variant is valid only in the foreign store");

    let mut module = enum_module();
    let option = module.enums.iter().nth(1).expect("two-variant option").0;
    append_variant_test_function(
        &mut module,
        "scoop.variant.foreign",
        LirType::Enum(option),
        invalid,
        LirType::I1,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("invalid enum1 variant 2"),
        "unexpected error: {error}"
    );
}

#[test]
fn variant_payload_projection_rejects_invalid_field_result_and_provenance() {
    let mut foreign = scoop_lir::EnumDefs::default();
    let foreign_shape = foreign.alloc(EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "foreign",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "foreign".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: vec![
                        EnumFieldRepr {
                            ty: LirType::I64,
                            offset: 8,
                        },
                        EnumFieldRepr {
                            ty: LirType::I64,
                            offset: 16,
                        },
                    ],
                    slot_offset: 8,
                    slot_size: 16,
                    slot_align: 8,
                    gc_free: true,
                },
            ],
            size: 24,
            align: 8,
        },
        scan: RefScan::None,
    });
    let foreign_variant = foreign
        .variant_ref(foreign_shape, 1)
        .expect("foreign variant");
    let invalid_field = foreign
        .variant_field_ref(foreign_variant, 1)
        .expect("field exists only in foreign store");

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    append_variant_projection_function(
        &mut module,
        "scoop.variant.bad_field",
        LirType::Enum(shape),
        invalid_field,
        LirType::I64,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_payload_project") && error.0.contains("invalid field 1"),
        "unexpected error: {error}"
    );

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let variant = module.enums.variant_ref(shape, 1).expect("Circle variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("Circle field");
    append_variant_projection_function(
        &mut module,
        "scoop.variant.bad_result",
        LirType::Enum(shape),
        field,
        LirType::I32,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_payload_project")
            && error.0.contains("result is i32, expected i64"),
        "unexpected error: {error}"
    );

    let mut module = enum_module_with(
        scoop_lir::NichePointerKind::Code,
        RefScan::None,
        LirType::I64,
    );
    let option = module.enums.iter().nth(1).expect("code niche").0;
    let variant = module
        .enums
        .variant_ref(option, 1)
        .expect("payload variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("code field");
    append_variant_projection_function(
        &mut module,
        "scoop.variant.bad_provenance",
        LirType::Enum(option),
        field,
        RAW_PTR,
    );
    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_payload_project")
            && error.0.contains("pointer provenance is raw, expected code"),
        "unexpected error: {error}"
    );
}

#[test]
fn variant_payload_projection_requires_matching_true_edge_dominance() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let variant = module.enums.variant_ref(shape, 1).expect("Circle variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("Circle field");
    let function_index = append_variant_projection_function(
        &mut module,
        "scoop.variant.undominated",
        LirType::Enum(shape),
        field,
        LirType::I64,
    );
    let function = &mut module.functions[function_index];
    let matched = function.blocks.iter().nth(1).expect("matched block").0;
    function.blocks[function.entry].terminator = Terminator::Br(matched);

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("variant_payload_project")
            && error.0.contains("not dominated")
            && error.0.contains("same operand"),
        "unexpected error: {error}"
    );
}

#[test]
fn redefining_a_tested_temp_invalidates_variant_dominance_fact() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("tagged enum").0;
    let variant = module.enums.variant_ref(shape, 1).expect("Circle variant");
    let field = module
        .enums
        .variant_field_ref(variant, 0)
        .expect("Circle field");
    let mut temps = Arena::default();
    let enum_value = temps.alloc(Temp {
        ty: LirType::Enum(shape),
    });
    let tested = temps.alloc(Temp { ty: LirType::I1 });
    let projected = temps.alloc(Temp { ty: LirType::I64 });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    let matched = blocks.alloc(BasicBlock {
        name: "matched".to_string(),
        instructions: vec![Instruction::VariantPayloadProject {
            out: projected,
            operand: Value::Temp(enum_value),
            field,
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(projected)),
        },
    });
    let missed = blocks.alloc(BasicBlock {
        name: "missed".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(signed64(0)),
        },
    });
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::EnumWrap {
                out: enum_value,
                variant,
                fields: vec![signed64(1)],
            },
            Instruction::VariantTest {
                out: tested,
                operand: Value::Temp(enum_value),
                variant,
            },
            // Reusing the same TempId is malformed SSA-ish LIR. More
            // importantly for this verifier, it changes the exact value that
            // the test fact described before control reaches the true edge.
            Instruction::EnumWrap {
                out: enum_value,
                variant,
                fields: vec![signed64(2)],
            },
        ],
        terminator: Terminator::CondBr {
            cond: Value::Temp(tested),
            then_block: matched,
            else_block: missed,
        },
    };
    module.functions.push(Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(Vec::new(), LirType::I64),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    });

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("defines temporary t0 more than once"),
        "unexpected error: {error}"
    );
}
