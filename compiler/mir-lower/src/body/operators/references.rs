use super::*;
use crate::closures::primitive::{BinaryReference, UnaryReference};

impl BodyLowerer<'_> {
    pub(crate) fn unary_reference_value(
        &mut self,
        operation: UnaryReference,
        operand: smir::Expr,
        result_type: &mir::Type,
    ) -> smir::Expr {
        match operation {
            UnaryReference::Primitive(kind) => {
                super::primitive::primitive_unary_value(kind, operand)
            }
            UnaryReference::Integer { kind, operation } => {
                super::primitive::integer_unary_value(kind, operation, operand)
            }
            UnaryReference::IntegerConversion(conversion) => smir::Expr::integer_conversion(
                mir::IntegerConversion::new(
                    lower_integer_kind(conversion.source),
                    lower_integer_kind(conversion.target_kind),
                ),
                operand,
            ),
            UnaryReference::Float { kind, operation } => smir::Expr::new(
                result_type.clone(),
                smir::ExprKind::FloatUnary {
                    kind,
                    operation,
                    operand: Box::new(operand),
                },
            ),
            UnaryReference::FloatConversion(conversion) => smir::Expr::new(
                result_type.clone(),
                smir::ExprKind::FloatConversion {
                    conversion: conversion.map_integer(lower_integer_kind),
                    operand: Box::new(operand),
                },
            ),
            UnaryReference::CharCode => char_code(operand),
            UnaryReference::CharFromCode => smir::Expr::new(
                result_type.clone(),
                smir::ExprKind::CharFromCodeUnchecked(Box::new(operand)),
            ),
        }
    }

    pub(crate) fn binary_reference_value(
        &mut self,
        operation: BinaryReference,
        lhs: smir::Expr,
        rhs: smir::Expr,
        result_type: &mir::Type,
        span: Span,
    ) -> smir::Expr {
        match operation {
            BinaryReference::Primitive(kind) => {
                super::primitive::primitive_binary_values(kind, lhs, rhs)
            }
            BinaryReference::Integer { kind, operation } => {
                self.integer_binary_values(kind, operation, lhs, rhs, span)
            }
            BinaryReference::Float { kind, operation } => smir::Expr::new(
                result_type.clone(),
                smir::ExprKind::FloatBinary {
                    kind,
                    operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            ),
            BinaryReference::CharEquals => smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    mir::IntegerKind::SIGNED_32,
                    mir::IntegerComparisonOperator::Equal,
                ),
                char_code(lhs),
                char_code(rhs),
            ),
            BinaryReference::CharCompareTo => smir::Expr::integer_compare_to(
                mir::IntegerCompareToOperation::new(mir::IntegerKind::SIGNED_32),
                char_code(lhs),
                char_code(rhs),
            ),
        }
    }
}

fn char_code(operand: smir::Expr) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Integer(mir::IntegerKind::SIGNED_32),
        smir::ExprKind::CharCode(Box::new(operand)),
    )
}
