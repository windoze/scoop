use super::*;

pub(super) fn floating_member(
    intrinsic: scoop_hir::FloatIntrinsicKind,
    args: Vec<smir::Expr>,
    result_type: &mir::Type,
) -> smir::Expr {
    let kind = match intrinsic {
        scoop_hir::FloatIntrinsicKind::Unary { kind, operation } => {
            let [operand] = args
                .try_into()
                .expect("a float unary member has one receiver");
            smir::ExprKind::FloatUnary {
                kind,
                operation,
                operand: Box::new(operand),
            }
        }
        scoop_hir::FloatIntrinsicKind::Binary { kind, operation } => {
            let [lhs, rhs] = args
                .try_into()
                .expect("a float binary member has two operands");
            smir::ExprKind::FloatBinary {
                kind,
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }
        }
        scoop_hir::FloatIntrinsicKind::Conversion(conversion) => {
            let [operand] = args
                .try_into()
                .expect("a float conversion has one receiver");
            smir::ExprKind::FloatConversion {
                conversion: conversion.map_integer(lower_integer_kind),
                operand: Box::new(operand),
            }
        }
    };
    smir::Expr::new(result_type.clone(), kind)
}
