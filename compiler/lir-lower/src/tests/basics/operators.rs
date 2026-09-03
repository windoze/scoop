use super::*;

#[test]
fn arithmetic_and_comparison_ops_map_to_lir_ops() {
    let mut b = Builder::new();
    let integer_cases = [
        (
            mir::BinOp::IntAdd,
            lir::BinOp::Add,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntSub,
            lir::BinOp::Sub,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntMul,
            lir::BinOp::Mul,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntDiv,
            lir::BinOp::SDiv,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntRem,
            lir::BinOp::SRem,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntCompareTo,
            lir::BinOp::SCompareTo,
            mir::Type::Int,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::UIntDiv,
            lir::BinOp::UDiv,
            mir::Type::UInt,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::UIntRem,
            lir::BinOp::URem,
            mir::Type::UInt,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::UIntCompareTo,
            lir::BinOp::UCompareTo,
            mir::Type::UInt,
            lir::LirType::I64,
        ),
        (
            mir::BinOp::IntLt,
            lir::BinOp::Lt,
            mir::Type::Int,
            lir::LirType::I1,
        ),
        (
            mir::BinOp::IntLe,
            lir::BinOp::Le,
            mir::Type::Int,
            lir::LirType::I1,
        ),
        (
            mir::BinOp::IntGt,
            lir::BinOp::Gt,
            mir::Type::Int,
            lir::LirType::I1,
        ),
        (
            mir::BinOp::IntGe,
            lir::BinOp::Ge,
            mir::Type::Int,
            lir::LirType::I1,
        ),
        (
            mir::BinOp::IntEq,
            lir::BinOp::Eq,
            mir::Type::Int,
            lir::LirType::I1,
        ),
        (
            mir::BinOp::IntNe,
            lir::BinOp::Ne,
            mir::Type::Int,
            lir::LirType::I1,
        ),
    ];
    let mut statements: Vec<mir::Statement> = integer_cases
        .iter()
        .map(|(mir_op, _, operand_ty, result_ty)| {
            expr_stmt(binary(
                *mir_op,
                expr(operand_ty.clone(), mir::ExprKind::IntLiteral(1)),
                expr(operand_ty.clone(), mir::ExprKind::IntLiteral(2)),
                if *result_ty == lir::LirType::I1 {
                    mir::Type::Boolean
                } else {
                    mir::Type::Int
                },
            ))
        })
        .collect();
    let bool_cases = [
        (mir::BinOp::BoolEq, lir::BinOp::Eq, lir::LirType::I1),
        (mir::BinOp::BoolNe, lir::BinOp::Ne, lir::LirType::I1),
    ];
    for (mir_op, _, _) in &bool_cases {
        statements.push(expr_stmt(binary(
            *mir_op,
            mir::Expr::bool(true),
            mir::Expr::bool(false),
            mir::Type::Boolean,
        )));
    }
    let main = b.main(Arena::new(), statements);
    let module = lower(&b.finish(main));

    let mut expected: Vec<(lir::BinOp, lir::LirType)> = integer_cases
        .iter()
        .map(|(_, lir_op, _, ty)| (*lir_op, ty.clone()))
        .collect();
    expected.extend(
        bool_cases
            .iter()
            .map(|(_, lir_op, ty)| (*lir_op, ty.clone())),
    );
    let function = &module.functions[0];
    let ops: Vec<(lir::BinOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| {
            let lir::Instruction::BinOp { out, op, .. } = instruction else {
                return None;
            };
            Some((*op, function.temps[*out].ty.clone()))
        })
        .collect();
    assert_eq!(ops, expected);
}

#[test]
fn unary_ops_map_to_lir_unops() {
    let mut b = Builder::new();
    let main = b.main(
        Arena::new(),
        vec![
            expr_stmt(expr(
                mir::Type::Int,
                mir::ExprKind::Unary {
                    op: mir::UnOp::IntNeg,
                    operand: Box::new(mir::Expr::int(1)),
                },
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
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let ops: Vec<(lir::UnOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| {
            let lir::Instruction::UnaryOp { out, op, .. } = instruction else {
                return None;
            };
            Some((*op, function.temps[*out].ty.clone()))
        })
        .collect();
    assert_eq!(
        ops,
        [
            (lir::UnOp::Neg, lir::LirType::I64),
            (lir::UnOp::Not, lir::LirType::I1),
        ]
    );
}
