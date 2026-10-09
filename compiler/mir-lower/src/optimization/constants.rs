use super::mir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Constant {
    Boolean(bool),
    Integer(mir::MirIntegerConstant),
}

impl Constant {
    pub fn expression(self) -> mir::Expr {
        match self {
            Self::Boolean(value) => {
                mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(value))
            }
            Self::Integer(value) => mir::Expr::integer(value),
        }
    }

    fn integer(self) -> Option<mir::MirIntegerConstant> {
        match self {
            Self::Integer(value) => Some(value),
            Self::Boolean(_) => None,
        }
    }
}

/// Only total scalar operations over already known, effect-free operands.
pub(super) fn evaluate(
    expression: &mir::Expr,
    value: impl Fn(&mir::Expr) -> Option<Constant>,
) -> Option<Constant> {
    use mir::ExprKind as E;
    let integer = |expr: &mir::Expr| value(expr)?.integer();
    Some(match &expression.kind {
        E::BoolLiteral(value) => Constant::Boolean(*value),
        E::IntegerLiteral(value) => Constant::Integer(*value),
        E::Retype { operand, .. } if operand.ty == expression.ty => value(operand)?,
        E::Unary {
            op: mir::UnOp::BoolNot,
            operand,
        } => {
            let Constant::Boolean(value) = value(operand)? else {
                return None;
            };
            Constant::Boolean(!value)
        }
        E::Binary {
            op: mir::BinOp::BoolEq | mir::BinOp::BoolNe,
            lhs,
            rhs,
        } => {
            let (Constant::Boolean(left), Constant::Boolean(right)) = (value(lhs)?, value(rhs)?)
            else {
                return None;
            };
            Constant::Boolean(
                if matches!(
                    expression.kind,
                    E::Binary {
                        op: mir::BinOp::BoolEq,
                        ..
                    }
                ) {
                    left == right
                } else {
                    left != right
                },
            )
        }
        E::IntegerUnary { operation, operand } => {
            let value = integer(operand)?.raw_bits();
            let bits = match operation.operator() {
                mir::IntegerUnaryOperator::Identity => value,
                mir::IntegerUnaryOperator::Negate => value.wrapping_neg(),
                mir::IntegerUnaryOperator::Increment => value.wrapping_add(1),
                mir::IntegerUnaryOperator::Decrement => value.wrapping_sub(1),
                mir::IntegerUnaryOperator::BitNot => !value,
            };
            constant(operation.kind(), bits)
        }
        E::IntegerBinary {
            operation,
            lhs,
            rhs,
        } => {
            let left = integer(lhs)?.raw_bits();
            let right = integer(rhs)?.raw_bits();
            let bits = match operation.operator() {
                mir::IntegerBinaryOperator::Add => left.wrapping_add(right),
                mir::IntegerBinaryOperator::Subtract => left.wrapping_sub(right),
                mir::IntegerBinaryOperator::Multiply => left.wrapping_mul(right),
                mir::IntegerBinaryOperator::BitAnd => left & right,
                mir::IntegerBinaryOperator::BitOr => left | right,
                mir::IntegerBinaryOperator::BitXor => left ^ right,
            };
            constant(operation.kind(), bits)
        }
        E::IntegerCompare {
            operation,
            lhs,
            rhs,
        } => {
            let order = integer(lhs)?
                .mathematical_value()
                .cmp(&integer(rhs)?.mathematical_value());
            Constant::Boolean(match operation.operator() {
                mir::IntegerComparisonOperator::LessThan => order.is_lt(),
                mir::IntegerComparisonOperator::LessThanOrEqual => !order.is_gt(),
                mir::IntegerComparisonOperator::GreaterThan => order.is_gt(),
                mir::IntegerComparisonOperator::GreaterThanOrEqual => !order.is_lt(),
                mir::IntegerComparisonOperator::Equal => order.is_eq(),
                mir::IntegerComparisonOperator::NotEqual => !order.is_eq(),
            })
        }
        E::IntegerCompareTo { lhs, rhs, .. } => {
            let result = match integer(lhs)?
                .mathematical_value()
                .cmp(&integer(rhs)?.mathematical_value())
            {
                std::cmp::Ordering::Less => -1_i64,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Constant::Integer(mir::MirIntegerConstant::Signed64(result as u64))
        }
        E::IntegerConversion {
            conversion,
            operand,
        } => constant(
            conversion.target_kind(),
            integer(operand)?.mathematical_value() as u64,
        ),
        // Guards, memory, allocation, runtime checks and calls keep their effects.
        _ => return None,
    })
}

fn constant(kind: mir::IntegerKind, bits: u64) -> Constant {
    Constant::Integer(
        mir::MirIntegerConstant::from_raw_bits(kind, bits & kind.width().raw_mask())
            .expect("masking retains only the exact integer width"),
    )
}
