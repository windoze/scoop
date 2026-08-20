use super::*;
use scoop_lir::LirIntegerConstant;

fn module_with_functions(functions: Vec<Function>) -> Module {
    let entry_effect = functions
        .first()
        .expect("integer test module has a function")
        .gc_effect;
    let mut module = values_module();
    module.functions = functions;
    module.output = scoop_lir::LirOutput::Executable {
        entry: local_function_ref(0, entry_effect),
    };
    module
}

fn constant_function(symbol: &str, constant: LirIntegerConstant) -> Function {
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::IntegerConst(constant)),
        },
    });
    Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(Vec::new(), constant.scalar_type()),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    }
}

fn instruction_module(
    params: Vec<LirType>,
    result_types: Vec<LirType>,
    make_instructions: impl FnOnce(&[TempId]) -> Vec<Instruction>,
) -> Module {
    let mut temps = Arena::default();
    let results = result_types
        .into_iter()
        .map(|ty| temps.alloc(Temp { ty }))
        .collect::<Vec<_>>();
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: make_instructions(&results),
        terminator: Terminator::Return { value: None },
    });
    module_with_functions(vec![Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::NoGc,
        signature: plain_scoop_signature(params, LirType::Void),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    }])
}

fn codegen_error(module: &Module) -> CodegenError {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    emit_llvm_module(&context, module, &machine, host_profile())
        .expect_err("integer fixture must be rejected")
}

#[test]
fn emits_all_eight_integer_kinds_with_exact_llvm_scalars_and_constants() {
    let cases = [
        ("integer_s8", LirIntegerConstant::Signed8(1), "i8", 1),
        ("integer_s16", LirIntegerConstant::Signed16(2), "i16", 2),
        ("integer_s32", LirIntegerConstant::Signed32(3), "i32", 3),
        ("integer_s64", LirIntegerConstant::Signed64(4), "i64", 4),
        ("integer_u8", LirIntegerConstant::Unsigned8(5), "i8", 5),
        ("integer_u16", LirIntegerConstant::Unsigned16(6), "i16", 6),
        ("integer_u32", LirIntegerConstant::Unsigned32(7), "i32", 7),
        ("integer_u64", LirIntegerConstant::Unsigned64(8), "i64", 8),
    ];
    let module = module_with_functions(
        cases
            .iter()
            .map(|(symbol, constant, _, _)| constant_function(symbol, *constant))
            .collect(),
    );
    let ir = ir_of(&module);
    for ((symbol, _, llvm_ty, value), function) in cases.into_iter().zip(&module.functions) {
        assert!(
            ir.contains(&format!(
                "define {llvm_ty} {}()",
                llvm_function_symbol(function)
            )),
            "missing exact scalar signature for {symbol}:\n{ir}"
        );
        assert!(
            ir.contains(&format!("ret {llvm_ty} {value}")),
            "missing typed constant for {symbol}:\n{ir}"
        );
    }
}

#[test]
fn c_bridge_uses_exact_stdint_spelling_for_all_integer_kinds() {
    let mut module = values_module();
    let offsets = [0, 2, 4, 8, 16, 18, 20, 24];
    let aligns = [1, 2, 4, 8, 1, 2, 4, 8];
    module.structs.alloc_c(
        crate::tests::test_physical_exact(
            "AllFixedWidthIntegers",
            scoop_identity::SourceNominalKind::Struct,
        ),
        "AllFixedWidthIntegers".to_string(),
        32,
        8,
        false,
        scoop_lir::LirCLayoutContract {
            aligned: scoop_lir::LirCLayoutValue::Natural,
            packed: scoop_lir::LirCLayoutValue::Natural,
        },
        IntegerKind::ALL
            .iter()
            .copied()
            .zip(offsets)
            .zip(aligns)
            .enumerate()
            .map(
                |(index, ((kind, offset), access_align))| scoop_lir::CStructField {
                    identity: test_field_identity(
                        "AllFixedWidthIntegers",
                        &format!("field{index}"),
                    ),
                    ty: scoop_lir::CType::Integer(kind),
                    layout: scoop_lir::FieldLayout {
                        offset,
                        access_align,
                    },
                },
            )
            .collect(),
    );
    install_test_native_function_contract(&mut module, 1);
    module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "integerWidths".to_string(),
            native_symbol: "native_integer_widths".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        call_plan: scoop_lir::CAbiCallPlan::StorageBridge {
            entry: Box::new(outbound_bridge(1)),
            result: scoop_identity::CResultAdaptation::Direct,
        },
        signature: scoop_lir::CFunctionType {
            params: IntegerKind::ALL
                .iter()
                .copied()
                .map(scoop_lir::CType::Integer)
                .collect(),
            return_type: scoop_lir::CReturnType::Void,
        },
    });

    let assertions = c_layout_assertions(&module).expect("integer C layout assertions");
    let bridge_sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module)
        .expect("C extern produces a bridge source");
    assert_eq!(bridge_sources.units().len(), 1);
    let source = bridge_sources.units()[0].source();
    assert!(source.contains("#include <stdint.h>"), "{source}");
    for (index, spelling) in [
        "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            assertions.contains(&format!("{spelling} _field_{index};")),
            "C-layout field {index} lost its exact integer spelling:\n{assertions}"
        );
    }
    assert!(
        source.contains(
            "extern void native_integer_widths(int8_t, int16_t, int32_t, int64_t, uint8_t, uint16_t, uint32_t, uint64_t);"
        ),
        "integer C spellings are incomplete:\n{source}"
    );

    let bridge_source = std::env::temp_dir().join(format!(
        "scoop_fixed_width_integer_bridge_{}.c",
        std::process::id()
    ));
    std::fs::write(&bridge_source, source).expect("write generated integer bridge C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(&bridge_source)
        .status()
        .expect("compile generated integer bridge C");
    std::fs::remove_file(&bridge_source).ok();
    assert!(status.success(), "generated integer bridge C must compile");
}

#[test]
fn compiler_pointer_shell_cannot_be_emitted_as_an_aggregate_struct() {
    let mut structs = scoop_lir::StructDefs::default();
    let shell = structs.alloc_intrinsic(
        crate::tests::test_physical_exact("Ptr<Int>", scoop_identity::SourceNominalKind::Struct),
        "Ptr<Int>".to_string(),
        8,
        8,
        scoop_lir::IntrinsicTypeRepresentation::Ptr {
            pointee: scoop_lir::LirDataPointee::Value(Box::new(LirType::I32)),
        },
    );
    let context = Context::create();
    let error = basic_ty(
        &context,
        &structs,
        &scoop_lir::EnumDefs::default(),
        host_managed_address_space(),
        &LirType::Struct(shell),
    )
    .expect_err("compiler pointer shells are not aggregate values");
    assert!(
        error.0.contains("cannot be emitted as an aggregate struct"),
        "unexpected error: {error}"
    );
}

#[test]
fn integer_comparisons_choose_signed_and_unsigned_predicates() {
    let module = instruction_module(vec![LirType::I8; 4], vec![LirType::I1; 2], |results| {
        vec![
            Instruction::IntegerCompare {
                out: results[0],
                kind: IntegerKind::SIGNED_8,
                comparison: IntegerComparison::Less,
                lhs: Value::Param(0),
                rhs: Value::Param(1),
            },
            Instruction::IntegerCompare {
                out: results[1],
                kind: IntegerKind::UNSIGNED_8,
                comparison: IntegerComparison::Less,
                lhs: Value::Param(2),
                rhs: Value::Param(3),
            },
        ]
    });
    let ir = ir_of(&module);
    assert!(ir.contains("icmp slt i8"), "{ir}");
    assert!(ir.contains("icmp ult i8"), "{ir}");
}

#[test]
fn compare_to_uses_each_operand_width_but_always_produces_i64() {
    let params = IntegerKind::ALL
        .iter()
        .flat_map(|kind| [kind.scalar_type(), kind.scalar_type()])
        .collect();
    let module = instruction_module(params, vec![LirType::I64; 8], |results| {
        IntegerKind::ALL
            .iter()
            .copied()
            .enumerate()
            .map(|(index, operand_kind)| Instruction::IntegerCompareTo {
                out: results[index],
                operand_kind,
                lhs: Value::Param((index * 2) as u32),
                rhs: Value::Param((index * 2 + 1) as u32),
            })
            .collect()
    });
    let ir = ir_of(&module);
    for llvm_ty in ["i8", "i16", "i32", "i64"] {
        assert!(ir.contains(&format!("icmp slt {llvm_ty}")), "{ir}");
        assert!(ir.contains(&format!("icmp ult {llvm_ty}")), "{ir}");
    }
    assert_eq!(ir.matches("select i1").count(), 16, "{ir}");
    assert!(
        ir.lines()
            .filter(|line| line.contains("select i1"))
            .all(|line| line.contains("i64")),
        "compareTo select results must all be i64:\n{ir}"
    );
}

#[test]
fn compare_to_rejects_a_non_long_result() {
    let module = instruction_module(
        vec![LirType::I8, LirType::I8],
        vec![LirType::I32],
        |results| {
            vec![Instruction::IntegerCompareTo {
                out: results[0],
                operand_kind: IntegerKind::SIGNED_8,
                lhs: Value::Param(0),
                rhs: Value::Param(1),
            }]
        },
    );
    let error = codegen_error(&module);
    assert!(
        error.0.contains("must produce canonical Long/i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn conversions_sign_extend_zero_extend_and_truncate_by_source_kind() {
    let module = instruction_module(
        vec![LirType::I8, LirType::I8, LirType::I64],
        vec![LirType::I64, LirType::I64, LirType::I8],
        |results| {
            vec![
                Instruction::IntegerConvert {
                    out: results[0],
                    source_kind: IntegerKind::SIGNED_8,
                    target_kind: IntegerKind::SIGNED_64,
                    operand: Value::Param(0),
                },
                Instruction::IntegerConvert {
                    out: results[1],
                    source_kind: IntegerKind::UNSIGNED_8,
                    target_kind: IntegerKind::UNSIGNED_64,
                    operand: Value::Param(1),
                },
                Instruction::IntegerConvert {
                    out: results[2],
                    source_kind: IntegerKind::SIGNED_64,
                    target_kind: IntegerKind::SIGNED_8,
                    operand: Value::Param(2),
                },
            ]
        },
    );
    let ir = ir_of(&module);
    assert!(ir.contains("sext i8"), "{ir}");
    assert!(ir.contains("zext i8"), "{ir}");
    assert!(ir.contains("trunc i64"), "{ir}");
}

#[test]
fn shifts_emit_left_arithmetic_right_and_logical_right() {
    let module = instruction_module(
        vec![LirType::I32, LirType::I32],
        vec![LirType::I32; 3],
        |results| {
            [
                IntegerShiftOperation::Left,
                IntegerShiftOperation::ArithmeticRight,
                IntegerShiftOperation::LogicalRight,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, operation)| Instruction::IntegerShift {
                out: results[index],
                kind: IntegerKind::SIGNED_32,
                operation,
                value: Value::Param(0),
                normalized_count: Value::Param(1),
            })
            .collect()
        },
    );
    let ir = ir_of(&module);
    assert!(ir.contains("shl i32"), "{ir}");
    assert!(ir.contains("ashr i32"), "{ir}");
    assert!(ir.contains("lshr i32"), "{ir}");
}

#[test]
fn safe_division_and_remainder_select_signedness_at_every_width() {
    let params = IntegerKind::ALL
        .iter()
        .flat_map(|kind| [kind.scalar_type(), kind.scalar_type()])
        .collect::<Vec<_>>();
    let result_types = IntegerKind::ALL
        .iter()
        .flat_map(|kind| [kind.scalar_type(), kind.scalar_type()])
        .collect::<Vec<_>>();
    let module = instruction_module(params, result_types, |results| {
        IntegerKind::ALL
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(index, kind)| {
                [
                    IntegerDivRemOperation::Divide,
                    IntegerDivRemOperation::Remainder,
                ]
                .into_iter()
                .enumerate()
                .map(move |(operation_index, operation)| {
                    Instruction::SafeIntegerDivRem {
                        out: results[index * 2 + operation_index],
                        kind,
                        operation,
                        lhs: Value::Param((index * 2) as u32),
                        rhs: Value::Param((index * 2 + 1) as u32),
                    }
                })
            })
            .collect()
    });
    let ir = ir_of(&module);
    for llvm_ty in ["i8", "i16", "i32", "i64"] {
        for opcode in ["sdiv", "srem", "udiv", "urem"] {
            assert!(
                ir.contains(&format!("{opcode} {llvm_ty}")),
                "missing {opcode} for {llvm_ty}:\n{ir}"
            );
        }
    }
}

#[test]
fn generic_binary_equality_rejects_source_integer_operands() {
    for op in [BinOp::Eq, BinOp::Ne] {
        let module = instruction_module(
            vec![LirType::I32, LirType::I32],
            vec![LirType::I1],
            |results| {
                vec![Instruction::BinOp {
                    out: results[0],
                    op,
                    lhs: Value::Param(0),
                    rhs: Value::Param(1),
                }]
            },
        );
        let error = codegen_error(&module);
        assert!(
            error.0.contains(&format!("generic equality {op:?}"))
                && error.0.contains("cannot consume source integer")
                && error.0.contains("i32"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn typed_integer_operation_rejects_same_width_constant_of_the_wrong_kind() {
    let module = instruction_module(Vec::new(), vec![LirType::I8], |results| {
        vec![Instruction::IntegerBinary {
            out: results[0],
            kind: IntegerKind::UNSIGNED_8,
            operation: IntegerBinaryOperation::Add,
            lhs: Value::IntegerConst(LirIntegerConstant::Signed8(1)),
            rhs: Value::IntegerConst(LirIntegerConstant::Unsigned8(2)),
        }]
    });
    let error = codegen_error(&module);
    assert!(
        error.0.contains("Add<UInt8>") && error.0.contains("requires i8, got i8"),
        "same-width signedness mismatch must still be rejected: {error}"
    );
}

#[test]
fn wrapping_s32_u32_s64_u64_arithmetic_has_no_llvm_overflow_flags() {
    let kinds = [
        IntegerKind::SIGNED_32,
        IntegerKind::UNSIGNED_32,
        IntegerKind::SIGNED_64,
        IntegerKind::UNSIGNED_64,
    ];
    let params = kinds
        .iter()
        .flat_map(|kind| [kind.scalar_type(), kind.scalar_type()])
        .collect();
    let result_types = kinds
        .iter()
        .flat_map(|kind| [kind.scalar_type(), kind.scalar_type(), kind.scalar_type()])
        .collect();
    let module = instruction_module(params, result_types, |results| {
        kinds
            .into_iter()
            .enumerate()
            .flat_map(|(kind_index, kind)| {
                [
                    IntegerBinaryOperation::Add,
                    IntegerBinaryOperation::Subtract,
                    IntegerBinaryOperation::Multiply,
                ]
                .into_iter()
                .enumerate()
                .map(move |(operation_index, operation)| {
                    Instruction::IntegerBinary {
                        out: results[kind_index * 3 + operation_index],
                        kind,
                        operation,
                        lhs: Value::Param((kind_index * 2) as u32),
                        rhs: Value::Param((kind_index * 2 + 1) as u32),
                    }
                })
            })
            .collect()
    });
    let ir = ir_of(&module);
    for llvm_ty in ["i32", "i64"] {
        for opcode in ["add", "sub", "mul"] {
            assert_eq!(
                ir.matches(&format!("{opcode} {llvm_ty}")).count(),
                2,
                "{ir}"
            );
        }
    }
    assert!(!ir.contains(" nsw "), "wrapping IR contains nsw:\n{ir}");
    assert!(!ir.contains(" nuw "), "wrapping IR contains nuw:\n{ir}");
}
