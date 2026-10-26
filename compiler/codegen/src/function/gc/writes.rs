use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Store a complete array element and record its reference range.
    /// No safepoint may occur between the caller's address calculation and completion.
    pub(in crate::function) fn heap_array_element_store(
        &self,
        array: PointerValue<'ctx>,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        stride: u64,
        scan: &RefScan,
    ) -> Result<(), CodegenError> {
        let mut references = Vec::new();
        flatten_ref_scan(scan, &mut references);
        references.sort_unstable();
        // Single/double-word elements keep the existing bounded-loop card
        // batching. Wide elements with one reference need only one comparison.
        if self.optimize_reference_writes
            && stride > 16
            && let [offset] = references.as_slice()
        {
            return self.store_with_reference_guard(array, address, value, *offset);
        }
        self.builder
            .build_store(address, value)
            .map_err(|error| CodegenError(format!("heap value store: {error}")))?;
        if let [offset] = references.as_slice() {
            // An array, including all of its elements, occupies one mapping.
            let region = self.card_region(array)?;
            let slot = self.byte_gep(address, *offset, "barrier_start")?;
            self.mark_card_in_region(slot, region)
        } else {
            self.heap_reference_barrier(address, &references)
        }
    }

    fn store_with_reference_guard(
        &self,
        array: PointerValue<'ctx>,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        offset: u64,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let slot = self.byte_gep(address, offset, "write.reference_slot")?;
        let reference = managed_ptr_ty(self.context, self.managed_address_space);
        let old = builder
            .build_load(reference, slot, "write.old_reference")
            .map_err(write_error)?
            .into_pointer_value();
        builder.build_store(address, value).map_err(write_error)?;
        // Ordinary store forwarding exposes the actual managed leaf, including
        // nested interface and MaybeUninit representations, without a new layout.
        let new = builder
            .build_load(reference, slot, "write.new_reference")
            .map_err(write_error)?
            .into_pointer_value();
        let changed = builder
            .build_int_compare(IntPredicate::NE, old, new, "write.changed")
            .map_err(write_error)?;
        let nonnull = builder
            .build_is_not_null(new, "write.nonnull")
            .map_err(write_error)?;
        let needed = builder
            .build_and(changed, nonnull, "write.needs_barrier")
            .map_err(write_error)?;
        let mark = self
            .context
            .append_basic_block(self.llvm_function, "write.barrier");
        let done = self
            .context
            .append_basic_block(self.llvm_function, "write.done");
        builder
            .build_conditional_branch(needed, mark, done)
            .map_err(write_error)?;
        builder.position_at_end(mark);
        let region = self.card_region(array)?;
        self.mark_card_in_region(slot, region)?;
        builder
            .build_unconditional_branch(done)
            .map_err(write_error)?;
        builder.position_at_end(done);
        Ok(())
    }

    pub(in crate::function) fn publish_release_ready(
        &self,
        object: PointerValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let context = self.context;
        let word = self.byte_gep(object, 8, "release_gc_word")?;
        builder
            .build_atomicrmw(
                AtomicRMWBinOp::Or,
                word,
                context.i64_type().const_int(4, false),
                AtomicOrdering::Release,
            )
            .map_err(|error| CodegenError(format!("publish release ready: {error}")))?;
        Ok(())
    }
}

fn write_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("guarded reference write: {error}"))
}
