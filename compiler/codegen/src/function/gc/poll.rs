//! Conditional managed poll with root materialization only on the slow edge.

use super::*;
use scoop_lir::RuntimeAbiSymbolV1 as Symbol;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn safepoint_poll(
        &mut self,
        site: &scoop_lir::ManagedPollSite,
    ) -> Result<(), CodegenError> {
        let target = self.function.call_targets.managed_targets.void[site.target].destination;
        if target
            != scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            )
        {
            return Err(CodegenError(format!(
                "managed poll @{} has a non-safepoint target",
                self.function.symbol()
            )));
        }
        let context = self.context;
        let builder = self.builder;
        let i32_ty = context.i32_type();
        let i64_ty = context.i64_type();
        let epoch = self.poll_atomic_load(
            i64_ty,
            self.poll_global(Symbol::GcEpoch, i64_ty.into()),
            "poll_epoch",
        )?;
        let phase = self.poll_atomic_load(
            i32_ty,
            self.poll_global(Symbol::WorldPhase, i32_ty.into()),
            "poll_phase",
        )?;
        let state = self.poll_state()?;
        let state_ty = context.struct_type(&[i32_ty.into(), i64_ty.into()], false);
        let mode_slot = builder
            .build_struct_gep(state_ty, state, 0, "poll_mode_slot")
            .map_err(poll_error)?;
        let observed_slot = builder
            .build_struct_gep(state_ty, state, 1, "poll_observed_slot")
            .map_err(poll_error)?;
        let mode = self.poll_atomic_load(i32_ty, mode_slot, "poll_mode")?;
        let observed = self.poll_atomic_load(i64_ty, observed_slot, "poll_observed")?;
        let running = builder
            .build_int_compare(
                IntPredicate::EQ,
                phase,
                i32_ty.const_int(scoop_lir::POLL_WORLD_RUNNING, false),
                "poll_running",
            )
            .map_err(poll_error)?;
        let managed = builder
            .build_int_compare(
                IntPredicate::EQ,
                mode,
                i32_ty.const_int(scoop_lir::POLL_THREAD_MANAGED, false),
                "poll_managed",
            )
            .map_err(poll_error)?;
        let current = builder
            .build_int_compare(IntPredicate::EQ, observed, epoch, "poll_current")
            .map_err(poll_error)?;
        let fast = builder
            .build_and(running, managed, "poll_active")
            .and_then(|active| builder.build_and(active, current, "poll_fast"))
            .map_err(poll_error)?;

        let safepoint = self.safepoint_id(site.safepoint);
        let slow = context.append_basic_block(
            self.llvm_function,
            &format!("poll.slow.{}", safepoint.get()),
        );
        let continuation = context.append_basic_block(
            self.llvm_function,
            &format!("poll.continue.{}", safepoint.get()),
        );
        builder
            .build_conditional_branch(fast, continuation, slow)
            .map_err(poll_error)?;
        builder.position_at_end(slow);
        let live = self.materialize_statepoint_live(&site.live, safepoint)?;
        let poll = self.runtime_fn(
            scoop_lir::RuntimeFunction::Managed(scoop_lir::ManagedRuntimeFunction::Safepoint)
                .symbol(),
            context.void_type().fn_type(&[], false),
        );
        for attribute in ["cold", "nounwind"] {
            poll.add_attribute(
                AttributeLoc::Function,
                context.create_enum_attribute(Attribute::get_named_enum_kind_id(attribute), 0),
            );
        }
        let call = builder.build_call(poll, &[], "").map_err(poll_error)?;
        self.apply_safepoint_id(call, safepoint);
        self.restore_statepoint_live(live, safepoint)?;
        builder
            .build_unconditional_branch(continuation)
            .map_err(poll_error)?;
        builder.position_at_end(continuation);
        Ok(())
    }

    fn poll_state(&mut self) -> Result<PointerValue<'ctx>, CodegenError> {
        if let Some(state) = self.cached_poll_state {
            return Ok(state);
        }
        let builder = self.context.create_builder();
        match self.entry_block.get_first_instruction() {
            Some(first) => builder.position_at(self.entry_block, &first),
            None => builder.position_at_end(self.entry_block),
        }
        let ptr = ptr_ty(self.context);
        let state = builder
            .build_load(
                ptr,
                self.poll_global(Symbol::PollState, ptr.into()),
                "poll_state",
            )
            .map_err(poll_error)?
            .into_pointer_value();
        self.cached_poll_state = Some(state);
        Ok(state)
    }

    fn poll_global(&self, symbol: Symbol, ty: BasicTypeEnum<'ctx>) -> PointerValue<'ctx> {
        self.llvm
            .get_global(symbol.logical_symbol())
            .unwrap_or_else(|| {
                let global = self.llvm.add_global(ty, None, symbol.logical_symbol());
                if symbol == Symbol::PollState {
                    global.set_thread_local(true);
                }
                global
            })
            .as_pointer_value()
    }

    fn poll_atomic_load(
        &self,
        ty: inkwell::types::IntType<'ctx>,
        pointer: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        let value = self
            .builder
            .build_load(ty, pointer, name)
            .map_err(poll_error)?;
        let instruction = value
            .as_instruction_value()
            .expect("atomic load is an instruction");
        instruction
            .set_atomic_ordering(AtomicOrdering::Acquire)
            .map_err(poll_error)?;
        instruction
            .set_alignment(ty.get_bit_width() / 8)
            .map_err(poll_error)?;
        Ok(value.into_int_value())
    }
}

fn poll_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("emit conditional managed poll: {error}"))
}
