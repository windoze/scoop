use super::*;

mod construction;

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
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: Vec::new().into(),
        terminator: Terminator::Unreachable,
    });
    let matched = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "matched".to_string(),
        instructions: Vec::new().into(),
        terminator: Terminator::Unreachable,
    });
    let missed = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "missed".to_string(),
        instructions: Vec::new().into(),
        terminator: Terminator::Unreachable,
    });
    blocks[entry] = BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: vec![Instruction::VariantTest {
            out: tested,
            operand: Value::Param(0),
            variant: field.variant(),
        }]
        .into(),
        terminator: Terminator::CondBr {
            cond: Value::Temp(tested),
            then_block: matched,
            else_block: missed,
        },
    };
    blocks[matched] = BasicBlock {
        terminator_origin: Default::default(),
        name: "matched".to_string(),
        instructions: vec![Instruction::VariantPayloadProject {
            out: projected,
            operand: Value::Param(0),
            field,
        }]
        .into(),
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
        terminator_origin: Default::default(),
        name: "missed".to_string(),
        instructions: Vec::new().into(),
        terminator: Terminator::Return {
            value: Some(fallback),
        },
    };
    let index = module.functions.len();
    module.functions.push(Function {
        debug_name: Default::default(),
        debug_origin: Default::default(),
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
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: vec![Instruction::VariantTest {
            out: tested,
            operand: Value::Param(0),
            variant,
        }]
        .into(),
        terminator: Terminator::Return {
            value: Some(Value::Temp(tested)),
        },
    });
    let index = module.functions.len();
    module.functions.push(Function {
        debug_name: Default::default(),
        debug_origin: Default::default(),
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
        scoop_lir::NullNicheKind::Managed,
        scoop_lir::NullNicheKind::Raw,
        scoop_lir::NullNicheKind::Code,
    ] {
        let scan = if kind == scoop_lir::NullNicheKind::Managed {
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
            &format!("scoop.variant.niche.{}", kind.dump()),
            LirType::Enum(option),
            field,
            kind.storage_type(),
        );
        append_variant_test_function(
            &mut module,
            &format!("scoop.variant.unit.{}", kind.dump()),
            LirType::Enum(option),
            unit,
            LirType::I1,
        );

        let ir = ir_of(&module);
        assert!(
            ir.contains("icmp ne") && ir.contains("icmp eq"),
            "niche payload/unit tests must be non-null/null comparisons:\n{ir}"
        );
        if kind == scoop_lir::NullNicheKind::Managed {
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
