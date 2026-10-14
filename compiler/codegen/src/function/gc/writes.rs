use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Store a complete value (including aggregates) and record its reference range.
    /// No safepoint may occur between the caller's address calculation and completion.
    pub(in crate::function) fn heap_store(
        &self,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        ty: &LirType,
    ) -> Result<(), CodegenError> {
        self.builder
            .build_store(address, value)
            .map_err(|error| CodegenError(format!("heap value store: {error}")))?;
        self.heap_value_barrier(address, ty)
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
