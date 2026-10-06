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

    pub(super) fn float_total_order(
        &self,
        kind: FloatKind,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
    ) -> Result<IntValue<'ctx>, BuilderError> {
        let lhs = self.float_order_key(kind, lhs)?;
        let rhs = self.float_order_key(kind, rhs)?;
        self.builder
            .build_int_compare(IntPredicate::ULE, lhs, rhs, "float.total_order")
    }

    fn float_order_key(
        &self,
        kind: FloatKind,
        operand: FloatValue<'ctx>,
    ) -> Result<IntValue<'ctx>, BuilderError> {
        let ty = bits_type(self.context, kind);
        let bits = self
            .builder
            .build_bit_cast(operand, ty, "float.bits")?
            .into_int_value();
        let negative = self.builder.build_int_compare(
            IntPredicate::SLT,
            bits,
            ty.const_zero(),
            "float.negative",
        )?;
        let complement = self.builder.build_not(bits, "float.complement")?;
        let positive = self.builder.build_xor(
            bits,
            ty.const_int(1_u64 << (kind.bits() - 1), false),
            "float.biased",
        )?;
        Ok(self
            .builder
            .build_select(negative, complement, positive, "float.order_key")?
            .into_int_value())
    }
}
