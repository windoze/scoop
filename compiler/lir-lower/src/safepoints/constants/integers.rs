use super::lir;
use lir::{IntegerKind, IntegerSignedness, IntegerWidth, LirIntegerConstant as C};

pub(super) fn result(
    instruction: &lir::Instruction,
    scalar: impl Fn(lir::Value) -> Option<lir::Value>,
) -> Option<(lir::TempId, lir::Value)> {
    let integer = |value, kind| match scalar(value)? {
        lir::Value::IntegerConst(value) if value.kind() == kind => Some(value),
        _ => None,
    };
    let (out, value) = match *instruction {
        lir::Instruction::IntegerUnary {
            out,
            kind,
            operation,
            operand,
        } => {
            let value = integer(operand, kind)?.raw_bits();
            let bits = match operation {
                lir::IntegerUnaryOperation::Plus => value,
                lir::IntegerUnaryOperation::Negate => value.wrapping_neg(),
                lir::IntegerUnaryOperation::BitwiseNot => !value,
            };
            (out, constant(kind, bits))
        }
        lir::Instruction::IntegerBinary {
            out,
            kind,
            operation,
            lhs,
            rhs,
        } => {
            let left = integer(lhs, kind)?.raw_bits();
            let right = integer(rhs, kind)?.raw_bits();
            let bits = match operation {
                lir::IntegerBinaryOperation::Add => left.wrapping_add(right),
                lir::IntegerBinaryOperation::Subtract => left.wrapping_sub(right),
                lir::IntegerBinaryOperation::Multiply => left.wrapping_mul(right),
                lir::IntegerBinaryOperation::BitwiseAnd => left & right,
                lir::IntegerBinaryOperation::BitwiseOr => left | right,
                lir::IntegerBinaryOperation::BitwiseXor => left ^ right,
            };
            (out, constant(kind, bits))
        }
        lir::Instruction::IntegerConvert {
            out,
            source_kind,
            target_kind,
            operand,
        } => {
            let value = integer(operand, source_kind)?;
            let bits = match source_kind.signedness() {
                IntegerSignedness::Signed => signed(value) as u64,
                IntegerSignedness::Unsigned => value.raw_bits(),
            };
            (out, constant(target_kind, bits))
        }
        lir::Instruction::IntegerShift {
            out,
            kind,
            operation,
            value,
            normalized_count,
        } => {
            let value = integer(value, kind)?;
            let count = integer(normalized_count, kind)?.raw_bits();
            if count >= u64::from(kind.width().bits()) {
                return None;
            }
            let bits = match operation {
                lir::IntegerShiftOperation::Left => value.raw_bits() << count,
                lir::IntegerShiftOperation::ArithmeticRight => (signed(value) >> count) as u64,
                lir::IntegerShiftOperation::LogicalRight => value.raw_bits() >> count,
            };
            (out, constant(kind, bits))
        }
        lir::Instruction::SafeIntegerDivRem {
            out,
            kind,
            operation,
            lhs,
            rhs,
        } => {
            let left = integer(lhs, kind)?;
            let right = integer(rhs, kind)?;
            let bits = match kind.signedness() {
                IntegerSignedness::Unsigned => match operation {
                    lir::IntegerDivRemOperation::Divide => {
                        left.raw_bits().checked_div(right.raw_bits())?
                    }
                    lir::IntegerDivRemOperation::Remainder => {
                        left.raw_bits().checked_rem(right.raw_bits())?
                    }
                },
                IntegerSignedness::Signed => {
                    if left.raw_bits() == 1 << (kind.width().bits() - 1) && signed(right) == -1 {
                        return None;
                    }
                    (match operation {
                        lir::IntegerDivRemOperation::Divide => {
                            signed(left).checked_div(signed(right))?
                        }
                        lir::IntegerDivRemOperation::Remainder => {
                            signed(left).checked_rem(signed(right))?
                        }
                    }) as u64
                }
            };
            (out, constant(kind, bits))
        }
        lir::Instruction::IntegerCompareTo {
            out,
            operand_kind,
            lhs,
            rhs,
        } => {
            let order = integer_order(
                operand_kind,
                integer(lhs, operand_kind)?,
                integer(rhs, operand_kind)?,
            )?;
            let value: i64 = match order {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            (out, C::Signed64(value as u64))
        }
        lir::Instruction::IntegerCompare {
            out,
            kind,
            comparison,
            lhs,
            rhs,
        } => {
            let order = integer_order(kind, integer(lhs, kind)?, integer(rhs, kind)?)?;
            let value = match comparison {
                lir::IntegerComparison::Less => order.is_lt(),
                lir::IntegerComparison::LessOrEqual => !order.is_gt(),
                lir::IntegerComparison::Greater => order.is_gt(),
                lir::IntegerComparison::GreaterOrEqual => !order.is_lt(),
                lir::IntegerComparison::Equal => order.is_eq(),
                lir::IntegerComparison::NotEqual => !order.is_eq(),
            };
            return Some((out, lir::Value::BoolConst(value)));
        }
        _ => return None,
    };
    Some((out, lir::Value::IntegerConst(value)))
}

fn signed(value: C) -> i64 {
    let shift = 64 - value.kind().width().bits();
    ((value.raw_bits() << shift) as i64) >> shift
}

pub(super) fn integer_order(kind: IntegerKind, left: C, right: C) -> Option<std::cmp::Ordering> {
    if left.kind() != kind || right.kind() != kind {
        return None;
    }
    Some(match kind.signedness() {
        IntegerSignedness::Unsigned => left.raw_bits().cmp(&right.raw_bits()),
        IntegerSignedness::Signed => signed(left).cmp(&signed(right)),
    })
}

fn constant(kind: IntegerKind, bits: u64) -> C {
    match (kind.signedness(), kind.width()) {
        (IntegerSignedness::Signed, IntegerWidth::W8) => C::Signed8(bits as u8),
        (IntegerSignedness::Signed, IntegerWidth::W16) => C::Signed16(bits as u16),
        (IntegerSignedness::Signed, IntegerWidth::W32) => C::Signed32(bits as u32),
        (IntegerSignedness::Signed, IntegerWidth::W64) => C::Signed64(bits),
        (IntegerSignedness::Unsigned, IntegerWidth::W8) => C::Unsigned8(bits as u8),
        (IntegerSignedness::Unsigned, IntegerWidth::W16) => C::Unsigned16(bits as u16),
        (IntegerSignedness::Unsigned, IntegerWidth::W32) => C::Unsigned32(bits as u32),
        (IntegerSignedness::Unsigned, IntegerWidth::W64) => C::Unsigned64(bits),
    }
}
