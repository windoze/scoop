use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_float_unary(
        &mut self,
        ty: &mir::Type,
        kind: mir::FloatKind,
        operation: mir::FloatUnaryOperator,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let operand = self.lower_expr(operand)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::FloatUnary {
            out,
            kind,
            operation,
            operand,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_float_binary(
        &mut self,
        ty: &mir::Type,
        kind: mir::FloatKind,
        operation: mir::FloatBinaryOperator,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::FloatBinary {
            out,
            kind,
            operation,
            lhs,
            rhs,
        });
        Ok(lir::Value::Temp(out))
    }

    pub(super) fn lower_float_conversion(
        &mut self,
        ty: &mir::Type,
        conversion: mir::MirFloatConversion,
        operand: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let operand = self.lower_expr(operand)?;
        let conversion = conversion.map_integer(integer_kind);
        let out_ty = self.value_type(ty);
        let out = self.new_temp(out_ty);
        self.push(lir::Instruction::FloatConversion {
            out,
            conversion,
            operand,
        });
        Ok(lir::Value::Temp(out))
    }
}
