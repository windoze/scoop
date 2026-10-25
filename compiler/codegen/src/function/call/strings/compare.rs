use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function::call) fn short_string_compare(
        &self,
        left: Value,
        right: Value,
    ) -> Result<Option<IntValue<'ctx>>, CodegenError> {
        let literal = |value| match value {
            Value::Global(id) => match &self.globals_arena[id].init {
                scoop_lir::GlobalInit::StringConst { value, .. } if value.len() <= 8 => {
                    Some(value.as_bytes())
                }
                _ => None,
            },
            _ => None,
        };
        let (source, text, reversed) = if let Some(text) = literal(right) {
            (left, text, false)
        } else if let Some(text) = literal(left) {
            (right, text, true)
        } else {
            return Ok(None);
        };
        let source = self.value(source)?.into_pointer_value();
        let length = self.string_byte_length(source)?;
        let size = self.context.i64_type().const_int(text.len() as u64, false);
        if text.is_empty() {
            let nonempty = self
                .builder
                .build_int_compare(IntPredicate::NE, length, size, "string.nonempty")
                .map_err(string_error)?;
            let result = self
                .builder
                .build_int_z_extend(nonempty, self.context.i64_type(), "string.empty_order")
                .map_err(string_error)?;
            return Ok(Some(if reversed {
                self.builder
                    .build_int_neg(result, "string.reverse_order")
                    .map_err(string_error)?
            } else {
                result
            }));
        }
        let same_length = self
            .builder
            .build_int_compare(IntPredicate::EQ, length, size, "string.same_length")
            .map_err(string_error)?;
        let short = self
            .context
            .append_basic_block(self.llvm_function, "string.short_compare");
        let fallback = self
            .context
            .append_basic_block(self.llvm_function, "string.compare_fallback");
        let done = self
            .context
            .append_basic_block(self.llvm_function, "string.order");
        self.builder
            .build_conditional_branch(same_length, short, fallback)
            .map_err(string_error)?;
        self.builder.position_at_end(short);
        // Build the unsigned big-endian word from exactly the guarded bytes.
        // LLVM may combine loads, but may not read beyond this valid range.
        let mut word = self.context.i64_type().const_zero();
        let mut expected = 0_u64;
        for (offset, &byte) in text.iter().enumerate() {
            let address = self.byte_gep(source, 24 + offset as u64, "string.compare_byte")?;
            let value = self
                .builder
                .build_load(self.context.i8_type(), address, "string.byte")
                .map_err(string_error)?
                .into_int_value();
            let value = self
                .builder
                .build_int_z_extend(value, self.context.i64_type(), "string.unsigned_byte")
                .map_err(string_error)?;
            word = self
                .builder
                .build_left_shift(
                    word,
                    self.context.i64_type().const_int(8, false),
                    "string.prefix",
                )
                .map_err(string_error)?;
            word = self
                .builder
                .build_or(word, value, "string.word")
                .map_err(string_error)?;
            expected = (expected << 8) | u64::from(byte);
        }
        let expected = self.context.i64_type().const_int(expected, false);
        let less = self
            .builder
            .build_int_compare(IntPredicate::ULT, word, expected, "string.less")
            .map_err(string_error)?;
        let greater = self
            .builder
            .build_int_compare(IntPredicate::UGT, word, expected, "string.greater")
            .map_err(string_error)?;
        let less = self
            .builder
            .build_int_z_extend(less, self.context.i64_type(), "string.less_word")
            .map_err(string_error)?;
        let greater = self
            .builder
            .build_int_z_extend(greater, self.context.i64_type(), "string.greater_word")
            .map_err(string_error)?;
        let order = if reversed {
            self.builder
                .build_int_sub(less, greater, "string.short_order")
        } else {
            self.builder
                .build_int_sub(greater, less, "string.short_order")
        }
        .map_err(string_error)?;
        self.builder
            .build_unconditional_branch(done)
            .map_err(string_error)?;
        self.builder.position_at_end(fallback);
        let left = self.value(left)?.into_pointer_value();
        let right = self.value(right)?.into_pointer_value();
        let callee = self.gc_leaf_fn(
            scoop_lir::RuntimeFunction::NoGc(scoop_lir::NoGcRuntimeFunction::StringCompare)
                .symbol(),
            self.context
                .i64_type()
                .fn_type(&[left.get_type().into(), right.get_type().into()], false),
        );
        let call = self
            .builder
            .build_call(callee, &[left.into(), right.into()], "string.shared_order")
            .map_err(string_error)?;
        self.apply_nounwind(call);
        let shared = call
            .try_as_basic_value()
            .basic()
            .expect("String comparison returns an integer")
            .into_int_value();
        self.builder
            .build_unconditional_branch(done)
            .map_err(string_error)?;
        self.builder.position_at_end(done);
        let result = self
            .builder
            .build_phi(self.context.i64_type(), "string.comparison")
            .map_err(string_error)?;
        result.add_incoming(&[(&order, short), (&shared, fallback)]);
        Ok(Some(result.as_basic_value().into_int_value()))
    }
}
