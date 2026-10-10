use super::*;

impl Expr {
    pub(crate) fn integer_unary(operation: mir::IntegerUnaryOperation, operand: Self) -> Self {
        Self::returning_operands([operand], |[operand]| {
            assert_eq!(operand.ty, mir::Type::Integer(operation.kind()));
            Self::new(
                mir::Type::Integer(operation.kind()),
                ExprKind::IntegerUnary {
                    operation,
                    operand: Box::new(operand),
                },
            )
        })
    }

    pub(crate) fn integer_binary(
        operation: mir::IntegerBinaryOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        Self::returning_operands([lhs, rhs], |[lhs, rhs]| {
            let ty = mir::Type::Integer(operation.kind());
            assert_eq!(lhs.ty, ty);
            assert_eq!(rhs.ty, ty);
            Self::new(
                ty,
                ExprKind::IntegerBinary {
                    operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            )
        })
    }

    pub(crate) fn safe_integer_div_rem(
        operation: mir::SafeIntegerDivRemOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        Self::returning_operands([lhs, rhs], |[lhs, rhs]| {
            let ty = mir::Type::Integer(operation.kind());
            assert_eq!(lhs.ty, ty);
            assert_eq!(rhs.ty, ty);
            Self::new(
                ty,
                ExprKind::SafeIntegerDivRem {
                    operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            )
        })
    }

    pub(crate) fn integer_compare(
        operation: mir::IntegerComparisonOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        Self::returning_operands([lhs, rhs], |[lhs, rhs]| {
            let operand_ty = mir::Type::Integer(operation.operand_kind());
            assert_eq!(lhs.ty, operand_ty);
            assert_eq!(rhs.ty, operand_ty);
            Self::new(
                mir::Type::Boolean,
                ExprKind::IntegerCompare {
                    operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            )
        })
    }

    pub(crate) fn integer_compare_to(
        operation: mir::IntegerCompareToOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        Self::returning_operands([lhs, rhs], |[lhs, rhs]| {
            let operand_ty = mir::Type::Integer(operation.operand_kind());
            assert_eq!(lhs.ty, operand_ty);
            assert_eq!(rhs.ty, operand_ty);
            Self::new(
                mir::Type::Integer(operation.result_kind()),
                ExprKind::IntegerCompareTo {
                    operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            )
        })
    }

    pub(crate) fn integer_shift(
        operation: mir::IntegerShiftOperation,
        value: Self,
        count: Self,
    ) -> Self {
        Self::returning_operands([value, count], |[value, count]| {
            assert_eq!(value.ty, mir::Type::Integer(operation.value_kind()));
            assert_eq!(count.ty, mir::Type::Integer(operation.count_kind()));
            Self::new(
                mir::Type::Integer(operation.value_kind()),
                ExprKind::IntegerShift {
                    operation,
                    value: Box::new(value),
                    count: Box::new(count),
                },
            )
        })
    }

    pub(crate) fn integer_conversion(conversion: mir::IntegerConversion, operand: Self) -> Self {
        Self::returning_operands([operand], |[operand]| {
            assert_eq!(operand.ty, mir::Type::Integer(conversion.source_kind()));
            Self::new(
                mir::Type::Integer(conversion.target_kind()),
                ExprKind::IntegerConversion {
                    conversion,
                    operand: Box::new(operand),
                },
            )
        })
    }
}
