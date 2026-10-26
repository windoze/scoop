//! Read the irreversible ready state before entering the collecting coordinator.

use super::*;
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn initialization_enter(
        &mut self,
        out: TempId,
        descriptor: Value,
        safepoint: scoop_lir::SafepointId,
        live: &scoop_lir::StatepointLiveSet,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let word = context.i64_type();
        let pointer = context.ptr_type(AddressSpace::default());
        let descriptor = self.value(descriptor)?.into_pointer_value();
        let types = RuntimeMetadataV1Types::new(context);
        let cell_slot = builder
            .build_struct_gep(
                types.initialization_unit_descriptor(),
                descriptor,
                5,
                "init.cell_slot",
            )
            .map_err(init_error)?;
        let cell = builder
            .build_load(pointer, cell_slot, "init.cell")
            .map_err(init_error)?
            .into_pointer_value();
        // State is the first, eight-byte-aligned word of the shared cell.
        let state = builder
            .build_load(word, cell, "init.state")
            .map_err(init_error)?
            .into_int_value();
        let load = state.as_instruction_value().expect("an atomic state load");
        load.set_alignment(8).map_err(init_error)?;
        load.set_atomic_ordering(AtomicOrdering::Acquire)
            .map_err(init_error)?;
        let ready = builder
            .build_int_compare(
                IntPredicate::EQ,
                state,
                word.const_int(scoop_lir::INITIALIZATION_CELL_READY, false),
                "init.ready",
            )
            .map_err(init_error)?;
        let fast = builder.get_insert_block().expect("the current call block");
        let slow = context.append_basic_block(
            self.llvm_function,
            &format!("init.slow.{}", safepoint.get()),
        );
        let continuation = context.append_basic_block(
            self.llvm_function,
            &format!("init.continue.{}", safepoint.get()),
        );
        builder
            .build_conditional_branch(ready, continuation, slow)
            .map_err(init_error)?;

        builder.position_at_end(slow);
        let live = self.materialize_statepoint_live(live, safepoint)?;
        let function = self.runtime_fn(
            scoop_lir::RuntimeFunction::Managed(
                scoop_lir::ManagedRuntimeFunction::InitializationEnter,
            )
            .symbol(),
            word.fn_type(&[pointer.into()], false),
        );
        let call = builder
            .build_call(function, &[descriptor.into()], "init.outcome")
            .map_err(init_error)?;
        self.apply_nounwind(call);
        self.apply_safepoint_id(call, safepoint);
        let outcome = call
            .try_as_basic_value()
            .basic()
            .expect("initialization enter returns its machine outcome")
            .into_int_value();
        self.restore_statepoint_live(live, safepoint)?;
        let slow = builder.get_insert_block().expect("the restored slow edge");
        builder
            .build_unconditional_branch(continuation)
            .map_err(init_error)?;
        builder.position_at_end(continuation);
        let result = builder.build_phi(word, "init.result").map_err(init_error)?;
        let ready = scoop_lir::MachineScalarValue::InitializationOutcome(
            scoop_lir::InitializationOutcome::Ready,
        );
        result.add_incoming(&[
            (&word.const_int(ready.raw_bits(), false), fast),
            (&outcome, slow),
        ]);
        self.temps.insert(out, result.as_basic_value());
        Ok(())
    }
}

fn init_error(error: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("initialization ready path: {error}"))
}
