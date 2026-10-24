use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Store a complete array element and record its reference range.
    /// No safepoint may occur between the caller's address calculation and completion.
    pub(in crate::function) fn heap_array_element_store(
        &self,
        array: PointerValue<'ctx>,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        ty: &LirType,
    ) -> Result<(), CodegenError> {
        self.builder
            .build_store(address, value)
            .map_err(|error| CodegenError(format!("heap value store: {error}")))?;
        let mut references = Vec::new();
        self.heap_reference_offsets(ty, 0, &mut references)?;
        references.sort_unstable();
        if let [offset] = references.as_slice() {
            // An array, including all of its elements, occupies one mapping.
            let region = self.card_region(array)?;
            let slot = self.byte_gep(address, *offset, "barrier_start")?;
            self.mark_card_in_region(slot, region)
        } else {
            self.heap_reference_barrier(address, &references)
        }
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
