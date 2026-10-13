use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn float_classify(
        &self,
        kind: FloatKind,
        operation: FloatUnaryOperator,
        operand: FloatValue<'ctx>,
    ) -> Result<IntValue<'ctx>, BuilderError> {
        let ty = bits_type(self.context, kind);
        let bits = self
            .builder
            .build_bit_cast(operand, ty, "float.bits")?
            .into_int_value();
        let magnitude_mask = (1_u64 << (kind.bits() - 1)) - 1;
        let magnitude =
            self.builder
                .build_and(bits, ty.const_int(magnitude_mask, false), "float.magnitude")?;
        let infinity = ty.const_int(
            match kind {
                FloatKind::F32 => 0x7f80_0000,
                FloatKind::F64 => 0x7ff0_0000_0000_0000,
            },
            false,
        );
        let predicate = match operation {
            FloatUnaryOperator::IsNaN => IntPredicate::UGT,
            FloatUnaryOperator::IsInfinite => IntPredicate::EQ,
            FloatUnaryOperator::IsFinite => IntPredicate::ULT,
            _ => unreachable!("classification has one of three predicates"),
        };
        self.builder
            .build_int_compare(predicate, magnitude, infinity, "float.classify")
    }

    pub(super) fn float_compare_to(
        &self,
        kind: FloatKind,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
    ) -> Result<IntValue<'ctx>, BuilderError> {
        let ty = self.context.i64_type();
        let equal = self.float_predicate(kind, FloatBinaryOperator::Equal, lhs, rhs)?;
        let less = self.float_predicate(kind, FloatBinaryOperator::Less, lhs, rhs)?;
        let nonzero = self.builder.build_select(
            less,
            ty.const_int(u64::MAX, true),
            ty.const_int(1, false),
            "float.order.nonzero",
        )?;
        Ok(self
            .builder
            .build_select(
                equal,
                ty.const_zero(),
                nonzero.into_int_value(),
                "float.order",
            )?
            .into_int_value())
    }

    pub(super) fn float_predicate(
        &self,
        kind: FloatKind,
        operation: FloatBinaryOperator,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
    ) -> Result<IntValue<'ctx>, BuilderError> {
        use FloatBinaryOperator as O;
        let builder = self.builder;
        let lhs_nan = self.float_classify(kind, FloatUnaryOperator::IsNaN, lhs)?;
        let rhs_nan = self.float_classify(kind, FloatUnaryOperator::IsNaN, rhs)?;
        let (predicate, nan_case) = match operation {
            O::Equal | O::NotEqual => (
                FloatPredicate::OEQ,
                builder.build_and(lhs_nan, rhs_nan, "float.both_nan")?,
            ),
            O::Less => (
                FloatPredicate::OLT,
                builder.build_and(
                    builder.build_not(lhs_nan, "float.lhs_number")?,
                    rhs_nan,
                    "float.nan_last",
                )?,
            ),
            O::LessEqual => (FloatPredicate::OLE, rhs_nan),
            O::Greater => (
                FloatPredicate::OGT,
                builder.build_and(
                    lhs_nan,
                    builder.build_not(rhs_nan, "float.rhs_number")?,
                    "float.nan_first",
                )?,
            ),
            O::GreaterEqual => (FloatPredicate::OGE, lhs_nan),
            _ => unreachable!("predicate instruction has a comparison operation"),
        };
        let numeric = builder.build_float_compare(predicate, lhs, rhs, "float.numeric_cmp")?;
        let result = builder.build_or(numeric, nan_case, "float.cmp")?;
        if operation == O::NotEqual {
            builder.build_not(result, "float.not_equal")
        } else {
            Ok(result)
        }
    }
}
