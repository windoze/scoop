//! Atomic heap writes keep the actual operation and conditional barrier together.

use super::*;
use inkwell::values::{AsValueRef, StructValue};
use scoop_lir::{AtomicLocation, AtomicValueKind};

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn heap_atomic_store(
        &self,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        location: AtomicLocation,
        order: AtomicOrdering,
    ) -> Result<(), CodegenError> {
        let instruction = self
            .builder
            .build_store(address, value)
            .map_err(|error| CodegenError(format!("atomic store: {error}")))?;
        instruction
            .set_atomic_ordering(order)
            .map_err(|error| CodegenError(format!("atomic store ordering: {error}")))?;
        instruction
            .set_alignment(location.kind.bytes())
            .map_err(|error| CodegenError(format!("atomic store alignment: {error}")))?;
        self.heap_value_barrier(address, &location.value_type())?;
        Ok(())
    }

    pub(in crate::function) fn heap_atomic_rmw(
        &self,
        address: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        location: AtomicLocation,
        operation: AtomicRMWBinOp,
        order: AtomicOrdering,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let old = if location.kind == AtomicValueKind::Reference {
            // Inkwell's atomicrmw wrapper only admits integer operands.
            // LLVM 22 supports pointer xchg without an integer carrier.
            let old = unsafe {
                llvm_sys::core::LLVMBuildAtomicRMW(
                    self.builder.as_mut_ptr(),
                    llvm_sys::LLVMAtomicRMWBinOp::LLVMAtomicRMWBinOpXchg,
                    address.as_value_ref(),
                    value.as_value_ref(),
                    order.into(),
                    0,
                )
            };
            unsafe { PointerValue::new(old) }.into()
        } else {
            self.builder
                .build_atomicrmw(operation, address, value.into_int_value(), order)
                .map_err(|error| CodegenError(format!("atomic RMW: {error}")))?
                .into()
        };
        self.heap_value_barrier(address, &location.value_type())?;

        Ok(old)
    }

    pub(in crate::function) fn heap_atomic_cmpxchg(
        &self,
        address: PointerValue<'ctx>,
        expected: BasicValueEnum<'ctx>,
        replacement: BasicValueEnum<'ctx>,
        location: AtomicLocation,
        orders: (AtomicOrdering, AtomicOrdering),
    ) -> Result<(StructValue<'ctx>, IntValue<'ctx>), CodegenError> {
        let (success_order, failure_order) = orders;
        let pair = self
            .builder
            .build_cmpxchg(address, expected, replacement, success_order, failure_order)
            .map_err(|error| CodegenError(format!("atomic compare exchange: {error}")))?;
        let success = self
            .builder
            .build_extract_value(pair, 1, "atomic_success")
            .map_err(|error| CodegenError(format!("atomic compare result: {error}")))?
            .into_int_value();
        if location.kind == AtomicValueKind::Reference {
            self.atomic_success_barrier(address, success)?;
        }

        Ok((pair, success))
    }

    fn atomic_success_barrier(
        &self,
        address: PointerValue<'ctx>,
        success: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let write = self
            .context
            .append_basic_block(self.llvm_function, "atomic_written");
        let done = self
            .context
            .append_basic_block(self.llvm_function, "atomic_done");
        self.builder
            .build_conditional_branch(success, write, done)
            .map_err(|error| CodegenError(format!("atomic write branch: {error}")))?;
        self.builder.position_at_end(write);
        self.heap_value_barrier(address, &scoop_lir::MANAGED_PTR)?;
        self.builder
            .build_unconditional_branch(done)
            .map_err(|error| CodegenError(format!("atomic write completion: {error}")))?;
        self.builder.position_at_end(done);
        Ok(())
    }
}
