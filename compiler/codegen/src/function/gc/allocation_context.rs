//! Reuse the attached mutator's stable address after the first actual allocation.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn allocation_context(&mut self) -> Result<PointerValue<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let symbol = scoop_lir::RuntimeAbiSymbolV1::AllocationContext.logical_symbol();
        let global = self.llvm.get_global(symbol).unwrap_or_else(|| {
            let global = self.llvm.add_global(ptr, None, symbol);
            global.set_thread_local(true);
            global
        });
        let load_context = |lazy: bool| {
            let value = builder
                .build_load(ptr, global.as_pointer_value(), "allocation_context")
                .map_err(context_error)?;
            if lazy {
                // Keep the costly TLS address resolution on the first-use edge.
                // This does not provide synchronization or cache mutable fields.
                value
                    .as_instruction_value()
                    .expect("a TLS load is an instruction")
                    .set_volatile(true)
                    .map_err(context_error)?;
            }
            Ok(value.into_pointer_value())
        };
        if !self.reuse_allocation_context {
            return load_context(false);
        }
        let slot = match self.allocation_context_slot {
            Some(slot) => slot,
            None => {
                let entry_builder = context.create_builder();
                match self.entry_block.get_first_instruction() {
                    Some(first) => entry_builder.position_at(self.entry_block, &first),
                    None => entry_builder.position_at_end(self.entry_block),
                }
                let slot = entry_builder
                    .build_alloca(ptr, "allocation_context_cache")
                    .map_err(context_error)?;
                entry_builder
                    .build_store(slot, ptr.const_null())
                    .map_err(context_error)?;
                self.allocation_context_slot = Some(slot);
                slot
            }
        };
        let cached = builder
            .build_load(ptr, slot, "allocation_context_cached")
            .map_err(context_error)?
            .into_pointer_value();
        let ready = builder
            .build_is_not_null(cached, "allocation_context_ready")
            .map_err(context_error)?;
        let previous = builder
            .get_insert_block()
            .expect("allocation context lookup has an active block");
        let load = context.append_basic_block(
            self.llvm_function,
            &format!("alloc.context.load.{}", self.allocation_index),
        );
        let continuation = context.append_basic_block(
            self.llvm_function,
            &format!("alloc.context.ready.{}", self.allocation_index),
        );
        builder
            .build_conditional_branch(ready, continuation, load)
            .map_err(context_error)?;
        builder.position_at_end(load);
        let loaded = load_context(true)?;
        builder.build_store(slot, loaded).map_err(context_error)?;
        builder
            .build_unconditional_branch(continuation)
            .map_err(context_error)?;
        builder.position_at_end(continuation);
        let phi = builder
            .build_phi(ptr, "allocation_context_value")
            .map_err(context_error)?;
        phi.add_incoming(&[(&cached, previous), (&loaded, load)]);
        Ok(phi.as_basic_value().into_pointer_value())
    }
}

fn context_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("reuse allocation context: {error}"))
}
