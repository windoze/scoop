use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_binary(
        &mut self,
        ty: &mir::Type,
        op: &mir::BinOp,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let lir_op = binary_op(*op);
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::BinOp {
            out,
            op: lir_op,
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_unary(
        &mut self,
        ty: &mir::Type,
        op: &mir::UnOp,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let lir_op = match op {
            mir::UnOp::BoolNot => lir::UnOp::Not,
        };
        let operand = self.lower_expr(operand)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::UnaryOp {
            out,
            op: lir_op,
            operand,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_unary(
        &mut self,
        ty: &mir::Type,
        operation: &mir::IntegerUnaryOperation,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let kind = integer_kind(operation.kind());
        assert_eq!(
            *ty,
            mir::Type::Integer(operation.kind()),
            "integer unary result preserves its exact kind",
        );
        let operand = self.lower_expr(operand)?;
        let out = self.new_temp(kind.scalar_type());
        match operation.operator() {
            mir::IntegerUnaryOperator::Identity => {
                self.push(lir::Instruction::IntegerUnary {
                    out,
                    kind,
                    operation: lir::IntegerUnaryOperation::Plus,
                    operand,
                });
            }
            mir::IntegerUnaryOperator::Negate => {
                self.push(lir::Instruction::IntegerUnary {
                    out,
                    kind,
                    operation: lir::IntegerUnaryOperation::Negate,
                    operand,
                });
            }
            mir::IntegerUnaryOperator::BitNot => {
                self.push(lir::Instruction::IntegerUnary {
                    out,
                    kind,
                    operation: lir::IntegerUnaryOperation::BitwiseNot,
                    operand,
                });
            }
            mir::IntegerUnaryOperator::Increment | mir::IntegerUnaryOperator::Decrement => {
                let one = mir::MirIntegerConstant::from_raw_bits(operation.kind(), 1)
                    .expect("one is representable by every integer kind");
                self.push(lir::Instruction::IntegerBinary {
                    out,
                    kind,
                    operation: if operation.operator() == mir::IntegerUnaryOperator::Increment {
                        lir::IntegerBinaryOperation::Add
                    } else {
                        lir::IntegerBinaryOperation::Subtract
                    },
                    lhs: operand,
                    rhs: lir::Value::IntegerConst(integer_constant(one)),
                });
            }
        }
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_binary(
        &mut self,
        ty: &mir::Type,
        operation: &mir::IntegerBinaryOperation,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let kind = integer_kind(operation.kind());
        assert_eq!(
            *ty,
            mir::Type::Integer(operation.kind()),
            "integer binary result preserves its exact kind",
        );
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out = self.new_temp(kind.scalar_type());
        self.push(lir::Instruction::IntegerBinary {
            out,
            kind,
            operation: integer_binary_op(operation.operator()),
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_safe_integer_div_rem(
        &mut self,
        ty: &mir::Type,
        operation: &mir::SafeIntegerDivRemOperation,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let kind = integer_kind(operation.kind());
        assert_eq!(
            *ty,
            mir::Type::Integer(operation.kind()),
            "safe integer div/rem result preserves its exact kind",
        );
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out = self.new_temp(kind.scalar_type());
        self.push(lir::Instruction::SafeIntegerDivRem {
            out,
            kind,
            operation: integer_div_rem_op(operation.operator()),
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_compare(
        &mut self,
        ty: &mir::Type,
        operation: &mir::IntegerComparisonOperation,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        assert_eq!(
            *ty,
            mir::Type::Boolean,
            "integer comparison returns Boolean"
        );
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out = self.new_temp(lir::LirType::I1);
        self.push(lir::Instruction::IntegerCompare {
            out,
            kind: integer_kind(operation.operand_kind()),
            comparison: integer_comparison(operation.operator()),
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_compare_to(
        &mut self,
        ty: &mir::Type,
        operation: &mir::IntegerCompareToOperation,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        assert_eq!(
            *ty,
            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
            "integer compareTo returns canonical Long",
        );
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out = self.new_temp(lir::LirType::I64);
        self.push(lir::Instruction::IntegerCompareTo {
            out,
            operand_kind: integer_kind(operation.operand_kind()),
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_shift(
        &mut self,
        ty: &mir::Type,
        operation: &mir::IntegerShiftOperation,
        value: &mir::Expr,
        count: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        assert_eq!(
            *ty,
            mir::Type::Integer(operation.value_kind()),
            "integer shift preserves its exact value kind",
        );
        assert_eq!(
            count.ty,
            mir::Type::Integer(operation.count_kind()),
            "integer shift count is normalized to the value's exact kind",
        );
        let value = self.lower_expr(value)?;
        let count = self.lower_expr(count)?;
        let kind = integer_kind(operation.value_kind());
        let out = self.new_temp(kind.scalar_type());
        self.push(lir::Instruction::IntegerShift {
            out,
            kind,
            operation: integer_shift_op(*operation),
            value,
            normalized_count: count,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_integer_conversion(
        &mut self,
        ty: &mir::Type,
        conversion: &mir::IntegerConversion,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        assert_eq!(
            *ty,
            mir::Type::Integer(conversion.target_kind()),
            "integer conversion has its exact target kind",
        );
        let operand = self.lower_expr(operand)?;
        let target_kind = integer_kind(conversion.target_kind());
        let out = self.new_temp(target_kind.scalar_type());
        self.push(lir::Instruction::IntegerConvert {
            out,
            source_kind: integer_kind(conversion.source_kind()),
            target_kind,
            operand,
        });
        Ok(lir::Value::Temp(out))
    }
}
