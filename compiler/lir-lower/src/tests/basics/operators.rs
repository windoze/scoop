use super::*;

fn integer_binary(kind: mir::IntegerKind, operator: mir::IntegerBinaryOperator) -> mir::Expr {
    mir::Expr::integer_binary(
        mir::IntegerBinaryOperation::new(kind, operator),
        integer_expr(kind, 6),
        integer_expr(kind, 2),
    )
}

fn safe_integer_div_rem(
    kind: mir::IntegerKind,
    operator: mir::SafeIntegerDivRemOperator,
) -> mir::Expr {
    mir::Expr::safe_integer_div_rem(
        mir::SafeIntegerDivRemOperation::new(kind, operator),
        integer_expr(kind, 6),
        integer_expr(kind, 2),
    )
}

#[test]
fn every_integer_kind_reaches_each_typed_lir_instruction_family() {
    let mut builder = Builder::new();
    let mut statements = Vec::new();
    for kind in mir::IntegerKind::ALL {
        statements.extend([
            expr_stmt(mir::Expr::integer_unary(
                mir::IntegerUnaryOperation::new(kind, mir::IntegerUnaryOperator::Negate),
                integer_expr(kind, 1),
            )),
            expr_stmt(integer_binary(kind, mir::IntegerBinaryOperator::Add)),
            expr_stmt(safe_integer_div_rem(
                kind,
                mir::SafeIntegerDivRemOperator::Divide,
            )),
            expr_stmt(mir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    kind,
                    mir::IntegerComparisonOperator::LessThan,
                ),
                integer_expr(kind, 1),
                integer_expr(kind, 2),
            )),
            expr_stmt(mir::Expr::integer_compare_to(
                mir::IntegerCompareToOperation::new(kind),
                integer_expr(kind, 1),
                integer_expr(kind, 2),
            )),
            expr_stmt(mir::Expr::integer_shift(
                mir::IntegerShiftOperation::new(kind, mir::IntegerShiftOperator::Left)
                    .expect("left shift is valid for every integer kind"),
                integer_expr(kind, 1),
                integer_expr(kind, 1),
            )),
            expr_stmt(mir::Expr::integer_conversion(
                mir::IntegerConversion::new(kind, mir::IntegerKind::UNSIGNED_64),
                integer_expr(kind, 1),
            )),
        ]);
    }
    let main = builder.main(Arena::new(), statements);
    let module = lower(builder.finish(main));
    let function = &module.functions[0];
    let instructions = &function.blocks[function.entry].instructions;

    let unary_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::IntegerUnary { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        unary_kinds,
        mir::IntegerKind::ALL.map(integer_kind).to_vec()
    );

    let safe_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::SafeIntegerDivRem {
                kind,
                operation: lir::IntegerDivRemOperation::Divide,
                ..
            } => Some(*kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(safe_kinds, unary_kinds);

    let comparison_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::IntegerCompare { kind, out, .. } => {
                assert_eq!(function.temps[*out].ty, lir::LirType::I1);
                Some(*kind)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(comparison_kinds, unary_kinds);

    let compare_to_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::IntegerCompareTo {
                operand_kind, out, ..
            } => {
                assert_eq!(function.temps[*out].ty, lir::LirType::I64);
                Some(*operand_kind)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(compare_to_kinds, unary_kinds);

    let shift_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::IntegerShift {
                kind,
                normalized_count,
                ..
            } => {
                assert_eq!(
                    function.value_ty(&module.globals, *normalized_count),
                    kind.scalar_type()
                );
                Some(*kind)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(shift_kinds, unary_kinds);

    let conversion_kinds = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::IntegerConvert {
                source_kind,
                target_kind: lir::IntegerKind::UNSIGNED_64,
                ..
            } => Some(*source_kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(conversion_kinds, unary_kinds);

    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, lir::Instruction::IntegerBinary { .. }))
            .count(),
        mir::IntegerKind::ALL.len(),
        "lir-lower must not recreate MIR's shift normalization",
    );
}

#[test]
fn all_integer_operators_map_without_generic_binop_or_unop_fallbacks() {
    let signed = mir::IntegerKind::SIGNED_8;
    let unsigned = mir::IntegerKind::UNSIGNED_8;
    let mut statements = Vec::new();
    for operator in [
        mir::IntegerUnaryOperator::Identity,
        mir::IntegerUnaryOperator::Negate,
        mir::IntegerUnaryOperator::Increment,
        mir::IntegerUnaryOperator::Decrement,
        mir::IntegerUnaryOperator::BitNot,
    ] {
        statements.push(expr_stmt(mir::Expr::integer_unary(
            mir::IntegerUnaryOperation::new(signed, operator),
            integer_expr(signed, 1),
        )));
    }
    for operator in [
        mir::IntegerBinaryOperator::Add,
        mir::IntegerBinaryOperator::Subtract,
        mir::IntegerBinaryOperator::Multiply,
        mir::IntegerBinaryOperator::BitAnd,
        mir::IntegerBinaryOperator::BitOr,
        mir::IntegerBinaryOperator::BitXor,
    ] {
        statements.push(expr_stmt(integer_binary(signed, operator)));
    }
    for operator in [
        mir::SafeIntegerDivRemOperator::Divide,
        mir::SafeIntegerDivRemOperator::Remainder,
    ] {
        statements.push(expr_stmt(safe_integer_div_rem(signed, operator)));
    }
    for operator in [
        mir::IntegerComparisonOperator::LessThan,
        mir::IntegerComparisonOperator::LessThanOrEqual,
        mir::IntegerComparisonOperator::GreaterThan,
        mir::IntegerComparisonOperator::GreaterThanOrEqual,
        mir::IntegerComparisonOperator::Equal,
        mir::IntegerComparisonOperator::NotEqual,
    ] {
        statements.push(expr_stmt(mir::Expr::integer_compare(
            mir::IntegerComparisonOperation::new(unsigned, operator),
            integer_expr(unsigned, 1),
            integer_expr(unsigned, 2),
        )));
    }
    for (kind, operator) in [
        (signed, mir::IntegerShiftOperator::Left),
        (signed, mir::IntegerShiftOperator::Right),
        (signed, mir::IntegerShiftOperator::UnsignedRight),
        (unsigned, mir::IntegerShiftOperator::Right),
    ] {
        statements.push(expr_stmt(mir::Expr::integer_shift(
            mir::IntegerShiftOperation::new(kind, operator).expect("valid shift pairing"),
            integer_expr(kind, 1),
            integer_expr(kind, 1),
        )));
    }
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), statements);
    let module = lower(builder.finish(main));
    let function = &module.functions[0];
    let instructions = &function.blocks[function.entry].instructions;

    assert!(!instructions.iter().any(|instruction| matches!(
        instruction,
        lir::Instruction::BinOp { .. } | lir::Instruction::UnaryOp { .. }
    )));
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, lir::Instruction::SafeIntegerDivRem { .. }))
            .count(),
        2
    );
    assert_eq!(
        instructions
            .iter()
            .filter_map(|instruction| match instruction {
                lir::Instruction::IntegerShift { operation, .. } => Some(*operation),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            lir::IntegerShiftOperation::Left,
            lir::IntegerShiftOperation::ArithmeticRight,
            lir::IntegerShiftOperation::LogicalRight,
            lir::IntegerShiftOperation::LogicalRight,
        ]
    );
}

#[test]
fn boolean_operations_stay_in_the_non_source_integer_family() {
    let mut builder = Builder::new();
    let main = builder.main(
        Arena::new(),
        vec![
            expr_stmt(binary(
                mir::BinOp::BoolEq,
                mir::Expr::bool(true),
                mir::Expr::bool(false),
                mir::Type::Boolean,
            )),
            expr_stmt(expr(
                mir::Type::Boolean,
                mir::ExprKind::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(mir::Expr::bool(true)),
                },
            )),
        ],
    );
    let module = lower(builder.finish(main));
    let function = &module.functions[0];
    let instructions = &function.blocks[function.entry].instructions;
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        lir::Instruction::BinOp {
            op: lir::BinOp::Eq,
            ..
        }
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        lir::Instruction::UnaryOp {
            op: lir::UnOp::Not,
            ..
        }
    )));
}

#[test]
fn reference_equality_maps_to_pointer_equality_without_integer_fallbacks() {
    let mut builder = Builder::new();
    let left = builder.string("left");
    let right = builder.string("right");
    let main = builder.main(
        Arena::new(),
        vec![
            expr_stmt(binary(
                mir::BinOp::RefEq,
                string_expr(left),
                string_expr(right),
                mir::Type::Boolean,
            )),
            expr_stmt(binary(
                mir::BinOp::RefNe,
                string_expr(left),
                string_expr(right),
                mir::Type::Boolean,
            )),
        ],
    );
    let module = lower(builder.finish(main));
    let function = &module.functions[0];
    let operations = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::BinOp { op, .. } => Some(*op),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(operations, [lir::BinOp::Eq, lir::BinOp::Ne]);
    assert!(
        !function.blocks[function.entry]
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, lir::Instruction::IntegerCompare { .. }))
    );
}
