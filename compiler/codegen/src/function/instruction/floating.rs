//! IEEE operations without fast-math flags or an intermediate precision.

use super::*;
use inkwell::FloatPredicate;
use inkwell::builder::BuilderError;
use inkwell::types::IntType;
use inkwell::values::FloatValue;
use scoop_lir::{FloatBinaryOperator, FloatKind, FloatUnaryOperator, LirFloatConversion};

mod conversions;
mod predicates;

fn float_error(error: BuilderError) -> CodegenError {
    CodegenError(format!("floating operation: {error}"))
}

fn bits_type(context: &Context, kind: FloatKind) -> IntType<'_> {
    match kind {
        FloatKind::F32 => context.i32_type(),
        FloatKind::F64 => context.i64_type(),
    }
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_float_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let (out, result) = match *instruction {
            Instruction::FloatUnary {
                out,
                kind,
                operation,
                operand,
            } => {
                let operand = self.value(operand)?.into_float_value();
                (
                    out,
                    self.float_unary(kind, operation, operand)
                        .map_err(float_error)?,
                )
            }
            Instruction::FloatBinary {
                out,
                kind,
                operation,
                lhs,
                rhs,
            } => {
                let lhs = self.value(lhs)?.into_float_value();
                let rhs = self.value(rhs)?.into_float_value();
                (
                    out,
                    self.float_binary(kind, operation, lhs, rhs)
                        .map_err(float_error)?,
                )
            }
            Instruction::FloatConversion {
                out,
                conversion,
                operand,
            } => {
                let operand = self.value(operand)?;
                (out, self.float_conversion(conversion, operand)?)
            }
            _ => unreachable!("floating dispatch contains only floating instructions"),
        };
        self.temps.insert(out, result);
        Ok(())
    }

    fn float_unary(
        &self,
        kind: FloatKind,
        operation: FloatUnaryOperator,
        operand: FloatValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, BuilderError> {
        use FloatUnaryOperator as O;
        Ok(match operation {
            O::Plus => operand.into(),
            // fneg only flips the sign bit, including for signaling NaNs.
            O::Negate => self.builder.build_float_neg(operand, "float.neg")?.into(),
            O::Increment => self
                .builder
                .build_float_add(
                    operand,
                    float_type(self.context, kind).const_float(1.0),
                    "float.inc",
                )?
                .into(),
            O::Decrement => self
                .builder
                .build_float_sub(
                    operand,
                    float_type(self.context, kind).const_float(1.0),
                    "float.dec",
                )?
                .into(),
            O::IsNaN | O::IsInfinite | O::IsFinite => {
                self.float_classify(kind, operation, operand)?.into()
            }
        })
    }

    fn float_binary(
        &self,
        kind: FloatKind,
        operation: FloatBinaryOperator,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, BuilderError> {
        use FloatBinaryOperator as O;
        let builder = self.builder;
        Ok(match operation {
            O::Add => builder.build_float_add(lhs, rhs, "float.add")?.into(),
            O::Subtract => builder.build_float_sub(lhs, rhs, "float.sub")?.into(),
            O::Multiply => builder.build_float_mul(lhs, rhs, "float.mul")?.into(),
            O::Divide => builder.build_float_div(lhs, rhs, "float.div")?.into(),
            O::Remainder => builder.build_float_rem(lhs, rhs, "float.rem")?.into(),
            O::TotalOrder => self.float_total_order(kind, lhs, rhs)?.into(),
            O::Equal | O::NotEqual | O::Less | O::LessEqual | O::Greater | O::GreaterEqual => {
                let predicate = match operation {
                    O::Equal => FloatPredicate::OEQ,
                    O::NotEqual => FloatPredicate::UNE,
                    O::Less => FloatPredicate::OLT,
                    O::LessEqual => FloatPredicate::OLE,
                    O::Greater => FloatPredicate::OGT,
                    O::GreaterEqual => FloatPredicate::OGE,
                    _ => unreachable!("comparison arm contains only predicates"),
                };
                builder
                    .build_float_compare(predicate, lhs, rhs, "float.cmp")?
                    .into()
            }
        })
    }
}
