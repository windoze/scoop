//! Complete small TLAB allocations through the shared runtime ABI view.

use super::*;

pub(super) fn allocation_context_type(context: &Context) -> inkwell::types::StructType<'_> {
    context.struct_type(&[ptr_ty(context).into(); 7], false)
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn finish_inline_allocation(
        &self,
        allocation: PointerValue<'ctx>,
        object: PointerValue<'ctx>,
        descriptor: PointerValue<'ctx>,
        size: IntValue<'ctx>,
        index: u32,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let i64_ty = self.context.i64_type();
        let constant = |value| i64_ty.const_int(value, false);
        builder
            .build_memset(object, 8, self.context.i8_type().const_zero(), size)
            .map_err(allocation_error)?;
        builder
            .build_store(object, descriptor)
            .map_err(allocation_error)?;

        let base = self.allocation_pointer(allocation, 2, "tlab_block_base")?;
        let base = builder
            .build_ptr_to_int(base, i64_ty, "tlab_base_int")
            .map_err(allocation_error)?;
        let address = builder
            .build_ptr_to_int(object, i64_ty, "tlab_object_int")
            .map_err(allocation_error)?;
        let offset = builder
            .build_int_sub(address, base, "tlab_offset")
            .map_err(allocation_error)?;
        let word = builder
            .build_right_shift(offset, constant(3), false, "tlab_object_word")
            .map_err(allocation_error)?;
        let sizes = self.allocation_pointer(allocation, 3, "tlab_sizes")?;
        // SAFETY: the successful bump lies within this block's size-unit array.
        let size_slot =
            unsafe { builder.build_gep(self.context.i16_type(), sizes, &[word], "tlab_size_slot") }
                .map_err(allocation_error)?;
        let units = builder
            .build_right_shift(size, constant(3), false, "tlab_size_units")
            .and_then(|units| {
                builder.build_int_truncate(units, self.context.i16_type(), "tlab_size_u16")
            })
            .map_err(allocation_error)?;
        builder
            .build_store(size_slot, units)
            .map_err(allocation_error)?;

        let last = builder
            .build_int_add(offset, size, "tlab_object_end")
            .and_then(|end| builder.build_int_sub(end, constant(1), "tlab_last_byte"))
            .map_err(allocation_error)?;
        let first_line = builder
            .build_right_shift(offset, constant(7), false, "tlab_first_line")
            .map_err(allocation_error)?;
        let last_line = builder
            .build_right_shift(last, constant(7), false, "tlab_last_line")
            .map_err(allocation_error)?;
        let first_word = builder
            .build_right_shift(first_line, constant(6), false, "tlab_first_line_word")
            .map_err(allocation_error)?;
        let last_word = builder
            .build_right_shift(last_line, constant(6), false, "tlab_last_line_word")
            .map_err(allocation_error)?;
        let first_bit = self.allocation_bit(first_line)?;
        let last_bit = self.allocation_bit(last_line)?;
        let same_word = builder
            .build_int_compare(
                IntPredicate::EQ,
                first_word,
                last_word,
                "tlab_same_line_word",
            )
            .map_err(allocation_error)?;
        let extra = builder
            .build_select(same_word, last_bit, constant(0), "tlab_same_word_bit")
            .map_err(allocation_error)?
            .into_int_value();
        let mask = builder
            .build_or(first_bit, extra, "tlab_first_line_mask")
            .map_err(allocation_error)?;
        let lines = self.allocation_pointer(allocation, 5, "tlab_occupied_lines")?;
        self.publish_allocation_bits(lines, first_word, mask, AtomicOrdering::Monotonic)?;

        // At most 128 aligned bytes cover two lines, including a word boundary.
        let second = self
            .context
            .append_basic_block(self.llvm_function, &format!("alloc.lines.second.{index}"));
        let complete = self
            .context
            .append_basic_block(self.llvm_function, &format!("alloc.lines.done.{index}"));
        builder
            .build_conditional_branch(same_word, complete, second)
            .map_err(allocation_error)?;
        builder.position_at_end(second);
        self.publish_allocation_bits(lines, last_word, last_bit, AtomicOrdering::Monotonic)?;
        builder
            .build_unconditional_branch(complete)
            .map_err(allocation_error)?;
        builder.position_at_end(complete);

        let starts = self.allocation_pointer(allocation, 4, "tlab_starts")?;
        let start_word = builder
            .build_right_shift(word, constant(6), false, "tlab_start_word")
            .map_err(allocation_error)?;
        self.publish_allocation_bits(
            starts,
            start_word,
            self.allocation_bit(word)?,
            AtomicOrdering::Release,
        )?;

        let counters = self.allocation_pointer(allocation, 6, "tlab_counters")?;
        for (field, increment) in [(0, constant(1)), (2, size), (1, constant(1)), (3, size)] {
            let slot = self.allocation_word(counters, constant(field))?;
            let value = self.allocation_atomic_load(slot)?;
            let next = builder
                .build_int_add(value, increment, "tlab_counter_next")
                .map_err(allocation_error)?;
            self.allocation_atomic_store(slot, next, AtomicOrdering::Monotonic)?;
        }
        Ok(())
    }

    fn allocation_pointer(
        &self,
        allocation: PointerValue<'ctx>,
        field: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let slot = self
            .builder
            .build_struct_gep(
                allocation_context_type(self.context),
                allocation,
                field,
                name,
            )
            .map_err(allocation_error)?;
        Ok(self
            .builder
            .build_load(ptr_ty(self.context), slot, name)
            .map_err(allocation_error)?
            .into_pointer_value())
    }

    fn allocation_bit(&self, index: IntValue<'ctx>) -> Result<IntValue<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let shift = self
            .builder
            .build_and(index, i64_ty.const_int(63, false), "tlab_bit_index")
            .map_err(allocation_error)?;
        self.builder
            .build_left_shift(i64_ty.const_int(1, false), shift, "tlab_bit")
            .map_err(allocation_error)
    }

    fn allocation_word(
        &self,
        storage: PointerValue<'ctx>,
        index: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: bitmap indexes are bounded by the successful block-sized bump;
        // counter indexes are the four fixed fields of the shared ABI view.
        unsafe {
            self.builder
                .build_gep(self.context.i64_type(), storage, &[index], "tlab_word_slot")
        }
        .map_err(allocation_error)
    }

    fn allocation_atomic_load(
        &self,
        slot: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        let value = self
            .builder
            .build_load(self.context.i64_type(), slot, "tlab_previous_word")
            .map_err(allocation_error)?;
        let instruction = value
            .as_instruction_value()
            .expect("an atomic load is an instruction");
        instruction.set_alignment(8).map_err(allocation_error)?;
        instruction
            .set_atomic_ordering(AtomicOrdering::Monotonic)
            .map_err(allocation_error)?;
        Ok(value.into_int_value())
    }

    fn allocation_atomic_store(
        &self,
        slot: PointerValue<'ctx>,
        value: IntValue<'ctx>,
        order: AtomicOrdering,
    ) -> Result<(), CodegenError> {
        let instruction = self
            .builder
            .build_store(slot, value)
            .map_err(allocation_error)?;
        instruction.set_alignment(8).map_err(allocation_error)?;
        instruction
            .set_atomic_ordering(order)
            .map_err(allocation_error)
    }

    fn publish_allocation_bits(
        &self,
        storage: PointerValue<'ctx>,
        word: IntValue<'ctx>,
        mask: IntValue<'ctx>,
        order: AtomicOrdering,
    ) -> Result<(), CodegenError> {
        let slot = self.allocation_word(storage, word)?;
        let previous = self.allocation_atomic_load(slot)?;
        let next = self
            .builder
            .build_or(previous, mask, "tlab_published_word")
            .map_err(allocation_error)?;
        self.allocation_atomic_store(slot, next, order)
    }
}

fn allocation_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("inline allocation metadata: {error}"))
}
