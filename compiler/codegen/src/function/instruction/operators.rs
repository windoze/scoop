use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_operator_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::BinOp { out, op, lhs, rhs } => {
                let lhs = self.value(*lhs)?;
                let rhs = self.value(*rhs)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: IntValue = match (lhs, rhs) {
                    (BasicValueEnum::PointerValue(lhs), BasicValueEnum::PointerValue(rhs)) => {
                        let predicate = match op {
                            BinOp::Eq => IntPredicate::EQ,
                            BinOp::Ne => IntPredicate::NE,
                            _ => {
                                return Err(CodegenError(format!(
                                    "pointer operands only support equality in @{}",
                                    function.symbol
                                )));
                            }
                        };
                        builder.build_int_compare(predicate, lhs, rhs, &name)
                    }
                    (BasicValueEnum::PointerValue(_), _) | (_, BasicValueEnum::PointerValue(_)) => {
                        return Err(CodegenError(format!(
                            "binary operands mix pointer and value types in @{}",
                            function.symbol
                        )));
                    }
                    (lhs, rhs) => {
                        let lhs = lhs.into_int_value();
                        let rhs = rhs.into_int_value();
                        match op {
                            BinOp::Add => builder.build_int_add(lhs, rhs, &name),
                            BinOp::Sub => builder.build_int_sub(lhs, rhs, &name),
                            BinOp::Mul => builder.build_int_mul(lhs, rhs, &name),
                            BinOp::SDiv => builder.build_int_signed_div(lhs, rhs, &name),
                            BinOp::SRem => builder.build_int_signed_rem(lhs, rhs, &name),
                            BinOp::UDiv => builder.build_int_unsigned_div(lhs, rhs, &name),
                            BinOp::URem => builder.build_int_unsigned_rem(lhs, rhs, &name),
                            BinOp::SCompareTo | BinOp::UCompareTo => (|| {
                                let (less, greater) = if *op == BinOp::SCompareTo {
                                    (IntPredicate::SLT, IntPredicate::SGT)
                                } else {
                                    (IntPredicate::ULT, IntPredicate::UGT)
                                };
                                let less = builder.build_int_compare(
                                    less,
                                    lhs,
                                    rhs,
                                    &format!("{name}.lt"),
                                )?;
                                let greater = builder.build_int_compare(
                                    greater,
                                    lhs,
                                    rhs,
                                    &format!("{name}.gt"),
                                )?;
                                let ty = lhs.get_type();
                                let positive = builder
                                    .build_select(
                                        greater,
                                        ty.const_int(1, false),
                                        ty.const_zero(),
                                        &format!("{name}.positive"),
                                    )?
                                    .into_int_value();
                                Ok::<_, inkwell::builder::BuilderError>(
                                    builder
                                        .build_select(less, ty.const_all_ones(), positive, &name)?
                                        .into_int_value(),
                                )
                            })(
                            ),
                            BinOp::Lt => {
                                builder.build_int_compare(IntPredicate::SLT, lhs, rhs, &name)
                            }
                            BinOp::Le => {
                                builder.build_int_compare(IntPredicate::SLE, lhs, rhs, &name)
                            }
                            BinOp::Gt => {
                                builder.build_int_compare(IntPredicate::SGT, lhs, rhs, &name)
                            }
                            BinOp::Ge => {
                                builder.build_int_compare(IntPredicate::SGE, lhs, rhs, &name)
                            }
                            BinOp::Eq => {
                                builder.build_int_compare(IntPredicate::EQ, lhs, rhs, &name)
                            }
                            BinOp::Ne => {
                                builder.build_int_compare(IntPredicate::NE, lhs, rhs, &name)
                            }
                        }
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            Instruction::UnaryOp { out, op, operand } => {
                let operand = self.value(*operand)?.into_int_value();
                let name = format!("t{}", out.into_raw().into_u32());
                let result = match op {
                    // Neg is `0 - x` (no dedicated LLVM neg instruction).
                    UnOp::Neg => {
                        builder.build_int_sub(operand.get_type().const_zero(), operand, &name)
                    }
                    // Not is `xor x, true`: flips the i1 bit directly.
                    UnOp::Not => {
                        builder.build_xor(operand, operand.get_type().const_all_ones(), &name)
                    }
                }
                .map_err(|e| {
                    CodegenError(format!("{op:?} @{symbol}: {e}", symbol = function.symbol))
                })?;
                self.temps.insert(*out, result.into());
            }
            _ => unreachable!("instruction dispatcher routes only operator instructions"),
        }
        Ok(())
    }
}
