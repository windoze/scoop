//! Reuse stable native thread addresses after their first actual use.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    fn thread_address(
        &mut self,
        symbol: scoop_lir::RuntimeAbiSymbolV1,
        name: &str,
        cache: Option<PointerValue<'ctx>>,
    ) -> Result<(PointerValue<'ctx>, Option<PointerValue<'ctx>>), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let symbol = symbol.logical_symbol();
        let global = self.llvm.get_global(symbol).unwrap_or_else(|| {
            let global = self.llvm.add_global(ptr, None, symbol);
            global.set_thread_local(true);
            global
        });
        let load_context = |lazy: bool| {
            let value = builder
                .build_load(ptr, global.as_pointer_value(), name)
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
        if !self.reuse_thread_addresses {
            return Ok((load_context(false)?, None));
        }
        let slot = match cache {
            Some(slot) => slot,
            None => {
                let entry_builder = context.create_builder();
                match self.entry_block.get_first_instruction() {
                    Some(first) => entry_builder.position_at(self.entry_block, &first),
                    None => entry_builder.position_at_end(self.entry_block),
                }
                let slot = entry_builder
                    .build_alloca(ptr, &format!("{name}_cache"))
                    .map_err(context_error)?;
                entry_builder
                    .build_store(slot, ptr.const_null())
                    .map_err(context_error)?;
                slot
            }
        };
        let cached = builder
            .build_load(ptr, slot, &format!("{name}_cached"))
            .map_err(context_error)?
            .into_pointer_value();
        let ready = builder
            .build_is_not_null(cached, &format!("{name}_ready"))
            .map_err(context_error)?;
        let previous = builder
            .get_insert_block()
            .expect("thread address lookup has an active block");
        let load = context.append_basic_block(
            self.llvm_function,
            &format!("{name}.load.{}", self.allocation_index),
        );
        let continuation = context.append_basic_block(
            self.llvm_function,
            &format!("{name}.ready.{}", self.allocation_index),
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
            .build_phi(ptr, &format!("{name}_value"))
            .map_err(context_error)?;
        phi.add_incoming(&[(&cached, previous), (&loaded, load)]);
        Ok((phi.as_basic_value().into_pointer_value(), Some(slot)))
    }

    pub(super) fn allocation_context(&mut self) -> Result<PointerValue<'ctx>, CodegenError> {
        let (address, cache) = self.thread_address(
            scoop_lir::RuntimeAbiSymbolV1::AllocationContext,
            "allocation_context",
            self.allocation_context_slot,
        )?;
        self.allocation_context_slot = cache;
        Ok(address)
    }

    pub(super) fn task_context_slot(&mut self) -> Result<PointerValue<'ctx>, CodegenError> {
        let (address, cache) = self.thread_address(
            scoop_lir::RuntimeAbiSymbolV1::ContextSlot,
            "task_context_slot",
            self.task_context_slot_cache,
        )?;
        self.task_context_slot_cache = cache;
        Ok(address)
    }
}

fn context_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("reuse thread address: {error}"))
}
