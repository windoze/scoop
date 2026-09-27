use super::*;

impl Writer<'_, '_> {
    pub(super) fn binop(&mut self, op: BinOp) -> Result {
        match op {
            BinOp::Eq => record!(self, 1;),
            BinOp::Ne => record!(self, 2;),
            BinOp::MachineEq(kind) => record!(self, 3; self.machine_kind(kind)),
        }
    }

    pub(super) fn integer_unary(&mut self, op: IntegerUnaryOperation) -> Result {
        self.u(match op {
            IntegerUnaryOperation::Plus => 1,
            IntegerUnaryOperation::Negate => 2,
            IntegerUnaryOperation::BitwiseNot => 3,
        })
    }

    pub(super) fn integer_binary(&mut self, op: IntegerBinaryOperation) -> Result {
        self.u(match op {
            IntegerBinaryOperation::Add => 1,
            IntegerBinaryOperation::Subtract => 2,
            IntegerBinaryOperation::Multiply => 3,
            IntegerBinaryOperation::BitwiseAnd => 4,
            IntegerBinaryOperation::BitwiseOr => 5,
            IntegerBinaryOperation::BitwiseXor => 6,
        })
    }

    pub(super) fn integer_divrem(&mut self, op: IntegerDivRemOperation) -> Result {
        self.u(match op {
            IntegerDivRemOperation::Divide => 1,
            IntegerDivRemOperation::Remainder => 2,
        })
    }

    pub(super) fn integer_comparison(&mut self, op: IntegerComparison) -> Result {
        self.u(match op {
            IntegerComparison::Less => 1,
            IntegerComparison::LessOrEqual => 2,
            IntegerComparison::Greater => 3,
            IntegerComparison::GreaterOrEqual => 4,
            IntegerComparison::Equal => 5,
            IntegerComparison::NotEqual => 6,
        })
    }

    pub(super) fn integer_shift(&mut self, op: IntegerShiftOperation) -> Result {
        self.u(match op {
            IntegerShiftOperation::Left => 1,
            IntegerShiftOperation::ArithmeticRight => 2,
            IntegerShiftOperation::LogicalRight => 3,
        })
    }
}
