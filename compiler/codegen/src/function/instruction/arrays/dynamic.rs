//! Allocate the zeroed backing of a managed initializer loop.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_dynamic_array_allocation(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let Instruction::ArrayAllocDynamic {
            out,
            count,
            array_type,
            overflow_message,
            safepoint,
            live,
        } = instruction
        else {
            unreachable!("the array dispatcher selects dynamic allocation")
        };
        if self.function.temps[*out].ty != scoop_lir::MANAGED_PTR
            || self.function.value_ty(self.globals_arena, *count) != LirType::I64
        {
            return Err(CodegenError(
                "dynamic array allocation requires a Long count and managed result".into(),
            ));
        }
        let (metadata, descriptor) = self.array_type(*array_type);
        let layout = metadata.layout.clone();
        // The managed negative-size guard dominates this operation. Unsigned
        // sizing checks below bound the count by the exact target layout.
        let count = self.value(*count)?.into_int_value();
        let bytes = self.checked_array_allocation_size(&layout, count, *overflow_message)?;
        let safepoint = self.safepoint_id(*safepoint);
        let live = self.materialize_statepoint_live(live, safepoint)?;
        let array =
            self.managed_alloc_value(descriptor.as_pointer_value(), bytes, safepoint, live)?;
        let size = self.byte_gep(array, 16, "array_size")?;
        self.builder
            .build_store(size, count)
            .map_err(|error| CodegenError(format!("store initialized array count: {error}")))?;
        self.temps.insert(*out, array.into());
        Ok(())
    }
}
