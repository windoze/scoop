use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_operator_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        match instruction {
            Instruction::BinOp { out, op, lhs, rhs } => {
                self.emit_non_source_integer_binary(*out, *op, *lhs, *rhs)?
            }
            Instruction::UnaryOp { out, op, operand } => {
                self.emit_boolean_unary(*out, *op, *operand)?
            }
            Instruction::IntegerUnary {
                out,
                kind,
                operation,
                operand,
            } => self.emit_integer_unary(*out, *kind, *operation, *operand)?,
            Instruction::IntegerBinary {
                out,
                kind,
                operation,
                lhs,
                rhs,
            } => self.emit_integer_binary(*out, *kind, *operation, *lhs, *rhs)?,
            Instruction::SafeIntegerDivRem {
                out,
                kind,
                operation,
                lhs,
                rhs,
            } => self.emit_safe_integer_div_rem(*out, *kind, *operation, *lhs, *rhs)?,
            Instruction::IntegerCompare {
                out,
                kind,
                comparison,
                lhs,
                rhs,
            } => self.emit_integer_compare(*out, *kind, *comparison, *lhs, *rhs)?,
            Instruction::IntegerCompareTo {
                out,
                operand_kind,
                lhs,
                rhs,
            } => self.emit_integer_compare_to(*out, *operand_kind, *lhs, *rhs)?,
            Instruction::IntegerShift {
                out,
                kind,
                operation,
                value,
                normalized_count,
            } => self.emit_integer_shift(*out, *kind, *operation, *value, *normalized_count)?,
            Instruction::IntegerConvert {
                out,
                source_kind,
                target_kind,
                operand,
            } => self.emit_integer_conversion(*out, *source_kind, *target_kind, *operand)?,
            _ => unreachable!("instruction dispatcher routes only operator instructions"),
        }
        Ok(())
    }

    fn emit_non_source_integer_binary(
        &mut self,
        out: TempId,
        op: BinOp,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), CodegenError> {
        let lhs_ty = self.function.value_ty(self.globals_arena, lhs);
        let rhs_ty = self.function.value_ty(self.globals_arena, rhs);
        if self.function.temps[out].ty != LirType::I1 || lhs_ty != rhs_ty {
            return Err(CodegenError(format!(
                "non-source-integer binary {op:?} in @{} has {} and {} operands with {} result",
                self.function.symbol(),
                lhs_ty.dump(),
                rhs_ty.dump(),
                self.function.temps[out].ty.dump(),
            )));
        }
        match op {
            BinOp::MachineEq(kind) => {
                let expected = LirType::MachineScalar(kind);
                if lhs_ty != expected {
                    return Err(CodegenError(format!(
                        "machine equality {kind:?} in @{} has operand type {}",
                        self.function.symbol(),
                        lhs_ty.dump(),
                    )));
                }
            }
            BinOp::Eq | BinOp::Ne => {
                if !matches!(lhs_ty, LirType::I1 | LirType::Ptr(_)) {
                    return Err(CodegenError(format!(
                        "generic equality {op:?} in @{} cannot consume source integer or aggregate type {}",
                        self.function.symbol(),
                        lhs_ty.dump(),
                    )));
                }
            }
        }

        let lhs = self.value(lhs)?;
        let rhs = self.value(rhs)?;
        let predicate = if op == BinOp::Ne {
            IntPredicate::NE
        } else {
            IntPredicate::EQ
        };
        let name = self.temp_name(out);
        let result = match (lhs, rhs) {
            (BasicValueEnum::PointerValue(lhs), BasicValueEnum::PointerValue(rhs)) => {
                self.builder.build_int_compare(predicate, lhs, rhs, &name)
            }
            (BasicValueEnum::IntValue(lhs), BasicValueEnum::IntValue(rhs)) => {
                self.builder.build_int_compare(predicate, lhs, rhs, &name)
            }
            _ => {
                return Err(CodegenError(format!(
                    "binary {op:?} in @{} received incompatible LLVM operands",
                    self.function.symbol()
                )));
            }
        }
        .map_err(|error| self.operator_error(op, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_boolean_unary(
        &mut self,
        out: TempId,
        op: UnOp,
        operand: Value,
    ) -> Result<(), CodegenError> {
        if self.function.value_ty(self.globals_arena, operand) != LirType::I1
            || self.function.temps[out].ty != LirType::I1
        {
            return Err(CodegenError(format!(
                "Boolean unary {op:?} in @{} requires i1 -> i1",
                self.function.symbol()
            )));
        }
        let operand = self.value(operand)?.into_int_value();
        let result = self
            .builder
            .build_xor(
                operand,
                operand.get_type().const_all_ones(),
                &self.temp_name(out),
            )
            .map_err(|error| self.operator_error(op, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_integer_unary(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        operation: IntegerUnaryOperation,
        operand: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand(operation, kind, operand)?;
        self.require_integer_result(operation, kind, out)?;
        let operand = self.value(operand)?.into_int_value();
        let name = self.temp_name(out);
        let result = match operation {
            IntegerUnaryOperation::Plus => {
                self.temps.insert(out, operand.into());
                return Ok(());
            }
            IntegerUnaryOperation::Negate => {
                self.builder
                    .build_int_sub(operand.get_type().const_zero(), operand, &name)
            }
            IntegerUnaryOperation::BitwiseNot => {
                self.builder
                    .build_xor(operand, operand.get_type().const_all_ones(), &name)
            }
        }
        .map_err(|error| self.operator_error(operation, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_integer_binary(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        operation: IntegerBinaryOperation,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand(operation, kind, lhs)?;
        self.require_integer_operand(operation, kind, rhs)?;
        self.require_integer_result(operation, kind, out)?;
        let lhs = self.value(lhs)?.into_int_value();
        let rhs = self.value(rhs)?.into_int_value();
        let name = self.temp_name(out);
        let result = match operation {
            IntegerBinaryOperation::Add => self.builder.build_int_add(lhs, rhs, &name),
            IntegerBinaryOperation::Subtract => self.builder.build_int_sub(lhs, rhs, &name),
            IntegerBinaryOperation::Multiply => self.builder.build_int_mul(lhs, rhs, &name),
            IntegerBinaryOperation::BitwiseAnd => self.builder.build_and(lhs, rhs, &name),
            IntegerBinaryOperation::BitwiseOr => self.builder.build_or(lhs, rhs, &name),
            IntegerBinaryOperation::BitwiseXor => self.builder.build_xor(lhs, rhs, &name),
        }
        .map_err(|error| self.operator_error(operation, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_safe_integer_div_rem(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        operation: IntegerDivRemOperation,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand(operation, kind, lhs)?;
        self.require_integer_operand(operation, kind, rhs)?;
        self.require_integer_result(operation, kind, out)?;
        let lhs = self.value(lhs)?.into_int_value();
        let rhs = self.value(rhs)?.into_int_value();
        let name = self.temp_name(out);
        let result = match (kind.signedness(), operation) {
            (IntegerSignedness::Signed, IntegerDivRemOperation::Divide) => {
                self.builder.build_int_signed_div(lhs, rhs, &name)
            }
            (IntegerSignedness::Signed, IntegerDivRemOperation::Remainder) => {
                self.builder.build_int_signed_rem(lhs, rhs, &name)
            }
            (IntegerSignedness::Unsigned, IntegerDivRemOperation::Divide) => {
                self.builder.build_int_unsigned_div(lhs, rhs, &name)
            }
            (IntegerSignedness::Unsigned, IntegerDivRemOperation::Remainder) => {
                self.builder.build_int_unsigned_rem(lhs, rhs, &name)
            }
        }
        .map_err(|error| self.operator_error(operation, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_integer_compare(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        comparison: IntegerComparison,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand(comparison, kind, lhs)?;
        self.require_integer_operand(comparison, kind, rhs)?;
        if self.function.temps[out].ty != LirType::I1 {
            return Err(CodegenError(format!(
                "integer comparison {comparison:?}<{}> in @{} must produce i1",
                kind.canonical_name(),
                self.function.symbol(),
            )));
        }
        let predicate = match (kind.signedness(), comparison) {
            (_, IntegerComparison::Equal) => IntPredicate::EQ,
            (_, IntegerComparison::NotEqual) => IntPredicate::NE,
            (IntegerSignedness::Signed, IntegerComparison::Less) => IntPredicate::SLT,
            (IntegerSignedness::Signed, IntegerComparison::LessOrEqual) => IntPredicate::SLE,
            (IntegerSignedness::Signed, IntegerComparison::Greater) => IntPredicate::SGT,
            (IntegerSignedness::Signed, IntegerComparison::GreaterOrEqual) => IntPredicate::SGE,
            (IntegerSignedness::Unsigned, IntegerComparison::Less) => IntPredicate::ULT,
            (IntegerSignedness::Unsigned, IntegerComparison::LessOrEqual) => IntPredicate::ULE,
            (IntegerSignedness::Unsigned, IntegerComparison::Greater) => IntPredicate::UGT,
            (IntegerSignedness::Unsigned, IntegerComparison::GreaterOrEqual) => IntPredicate::UGE,
        };
        let lhs = self.value(lhs)?.into_int_value();
        let rhs = self.value(rhs)?.into_int_value();
        let result = self
            .builder
            .build_int_compare(predicate, lhs, rhs, &self.temp_name(out))
            .map_err(|error| self.operator_error(comparison, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_integer_compare_to(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand("compareTo", kind, lhs)?;
        self.require_integer_operand("compareTo", kind, rhs)?;
        if self.function.temps[out].ty != LirType::I64 {
            return Err(CodegenError(format!(
                "integer compareTo<{}> in @{} must produce canonical Long/i64",
                kind.canonical_name(),
                self.function.symbol(),
            )));
        }
        let lhs = self.value(lhs)?.into_int_value();
        let rhs = self.value(rhs)?.into_int_value();
        let (less_predicate, greater_predicate) = match kind.signedness() {
            IntegerSignedness::Signed => (IntPredicate::SLT, IntPredicate::SGT),
            IntegerSignedness::Unsigned => (IntPredicate::ULT, IntPredicate::UGT),
        };
        let name = self.temp_name(out);
        let less = self
            .builder
            .build_int_compare(less_predicate, lhs, rhs, &format!("{name}.lt"))
            .map_err(|error| self.operator_error("compareTo less", error))?;
        let greater = self
            .builder
            .build_int_compare(greater_predicate, lhs, rhs, &format!("{name}.gt"))
            .map_err(|error| self.operator_error("compareTo greater", error))?;
        let result_ty = self.context.i64_type();
        let nonnegative = self
            .builder
            .build_select(
                greater,
                result_ty.const_int(1, false),
                result_ty.const_zero(),
                &format!("{name}.nonnegative"),
            )
            .map_err(|error| self.operator_error("compareTo positive select", error))?
            .into_int_value();
        let result = self
            .builder
            .build_select(less, result_ty.const_all_ones(), nonnegative, &name)
            .map_err(|error| self.operator_error("compareTo result select", error))?;
        self.temps.insert(out, result);
        Ok(())
    }

    fn emit_integer_shift(
        &mut self,
        out: TempId,
        kind: IntegerKind,
        operation: IntegerShiftOperation,
        value: Value,
        normalized_count: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand(operation, kind, value)?;
        self.require_integer_operand(operation, kind, normalized_count)?;
        self.require_integer_result(operation, kind, out)?;
        let value = self.value(value)?.into_int_value();
        let count = self.value(normalized_count)?.into_int_value();
        let name = self.temp_name(out);
        let result = match operation {
            IntegerShiftOperation::Left => self.builder.build_left_shift(value, count, &name),
            IntegerShiftOperation::ArithmeticRight => {
                self.builder.build_right_shift(value, count, true, &name)
            }
            IntegerShiftOperation::LogicalRight => {
                self.builder.build_right_shift(value, count, false, &name)
            }
        }
        .map_err(|error| self.operator_error(operation, error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn emit_integer_conversion(
        &mut self,
        out: TempId,
        source_kind: IntegerKind,
        target_kind: IntegerKind,
        operand: Value,
    ) -> Result<(), CodegenError> {
        self.require_integer_operand("conversion", source_kind, operand)?;
        self.require_integer_result("conversion", target_kind, out)?;
        let operand = self.value(operand)?.into_int_value();
        let source_bits = source_kind.width().bits();
        let target_bits = target_kind.width().bits();
        if source_bits == target_bits {
            self.temps.insert(out, operand.into());
            return Ok(());
        }
        let target = integer_ty(self.context, target_kind.width());
        let name = self.temp_name(out);
        let result = if source_bits > target_bits {
            self.builder.build_int_truncate(operand, target, &name)
        } else if source_kind.signedness() == IntegerSignedness::Signed {
            self.builder.build_int_s_extend(operand, target, &name)
        } else {
            self.builder.build_int_z_extend(operand, target, &name)
        }
        .map_err(|error| self.operator_error("integer conversion", error))?;
        self.temps.insert(out, result.into());
        Ok(())
    }

    fn require_integer_operand(
        &self,
        operation: impl std::fmt::Debug,
        kind: IntegerKind,
        value: Value,
    ) -> Result<(), CodegenError> {
        let actual = self.function.value_ty(self.globals_arena, value);
        let exact_constant = match value {
            Value::IntegerConst(constant) => constant.kind() == kind,
            _ => true,
        };
        if actual != kind.scalar_type() || !exact_constant {
            return Err(CodegenError(format!(
                "integer operation {operation:?}<{}> in @{} requires {}, got {}",
                kind.canonical_name(),
                self.function.symbol(),
                kind.scalar_type().dump(),
                actual.dump(),
            )));
        }
        Ok(())
    }

    fn require_integer_result(
        &self,
        operation: impl std::fmt::Debug,
        kind: IntegerKind,
        out: TempId,
    ) -> Result<(), CodegenError> {
        let actual = &self.function.temps[out].ty;
        if actual != &kind.scalar_type() {
            return Err(CodegenError(format!(
                "integer operation {operation:?}<{}> in @{} must produce {}, got {}",
                kind.canonical_name(),
                self.function.symbol(),
                kind.scalar_type().dump(),
                actual.dump(),
            )));
        }
        Ok(())
    }

    fn temp_name(&self, out: TempId) -> String {
        format!("t{}", out.into_raw().into_u32())
    }

    fn operator_error(
        &self,
        operation: impl std::fmt::Debug,
        error: inkwell::builder::BuilderError,
    ) -> CodegenError {
        CodegenError(format!(
            "{operation:?} @{}: {error}",
            self.function.symbol()
        ))
    }
}
