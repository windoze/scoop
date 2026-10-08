use super::*;
use inkwell::values::AsValueRef;
use scoop_lir::{
    AtomicCompareExchangeResult, AtomicLocation, AtomicMemoryOrder, AtomicRmwOperation,
    AtomicValueKind,
};

mod values;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_atomic_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        match instruction {
            Instruction::AtomicLoad {
                out,
                location,
                order,
            } => {
                self.require_atomic_result(*out, location.value_type())?;
                let address = self.atomic_address(*location)?;
                let value = self
                    .builder
                    .build_load(self.atomic_storage_type(*location)?, address, "atomic_load")
                    .map_err(|error| CodegenError(format!("atomic load: {error}")))?;
                let instruction = value
                    .as_instruction_value()
                    .expect("load is an instruction");
                instruction
                    .set_atomic_ordering(llvm_order(order.memory_order()))
                    .map_err(|error| CodegenError(format!("atomic load ordering: {error}")))?;
                instruction
                    .set_alignment(location.kind.bytes())
                    .map_err(|error| CodegenError(format!("atomic load alignment: {error}")))?;
                let value = self.atomic_result(*location, value)?;
                self.temps.insert(*out, value);
            }
            Instruction::AtomicStore {
                location,
                value,
                order,
            } => {
                let address = self.atomic_address(*location)?;
                let value = self.atomic_operand(*location, *value)?;
                let instruction = self
                    .builder
                    .build_store(address, value)
                    .map_err(|error| CodegenError(format!("atomic store: {error}")))?;
                instruction
                    .set_atomic_ordering(llvm_order(order.memory_order()))
                    .map_err(|error| CodegenError(format!("atomic store ordering: {error}")))?;
                instruction
                    .set_alignment(location.kind.bytes())
                    .map_err(|error| CodegenError(format!("atomic store alignment: {error}")))?;
                self.heap_value_barrier(address, &location.value_type())?;
            }
            Instruction::AtomicRmw {
                out,
                location,
                value,
                operation,
                order,
            } => {
                if !operation.accepts(location.kind) {
                    return Err(CodegenError(format!(
                        "atomic {operation:?} cannot operate on {:?}",
                        location.kind
                    )));
                }
                self.require_atomic_result(*out, location.value_type())?;
                let address = self.atomic_address(*location)?;
                let value = self.atomic_operand(*location, *value)?;
                let old = if location.kind == AtomicValueKind::Reference {
                    // Inkwell's atomicrmw wrapper only admits integer operands.
                    // LLVM 22 supports pointer xchg without an integer carrier.
                    let old = unsafe {
                        llvm_sys::core::LLVMBuildAtomicRMW(
                            self.builder.as_mut_ptr(),
                            llvm_sys::LLVMAtomicRMWBinOp::LLVMAtomicRMWBinOpXchg,
                            address.as_value_ref(),
                            value.as_value_ref(),
                            llvm_order(*order).into(),
                            0,
                        )
                    };
                    unsafe { PointerValue::new(old) }.into()
                } else {
                    self.builder
                        .build_atomicrmw(
                            llvm_rmw(*operation),
                            address,
                            value.into_int_value(),
                            llvm_order(*order),
                        )
                        .map_err(|error| CodegenError(format!("atomic RMW: {error}")))?
                        .into()
                };
                self.heap_value_barrier(address, &location.value_type())?;
                let old = self.atomic_result(*location, old)?;
                self.temps.insert(*out, old);
            }
            Instruction::AtomicCmpXchg {
                out,
                location,
                expected,
                replacement,
                result,
                order,
            } => {
                let result_type = match result {
                    AtomicCompareExchangeResult::ObservedValue => location.value_type(),
                    AtomicCompareExchangeResult::Success => LirType::I1,
                };
                self.require_atomic_result(*out, result_type)?;
                let address = self.atomic_address(*location)?;
                let expected = self.atomic_operand(*location, *expected)?;
                let replacement = self.atomic_operand(*location, *replacement)?;
                let (success_order, failure_order) = order.memory_orders();
                let pair = self
                    .builder
                    .build_cmpxchg(
                        address,
                        expected,
                        replacement,
                        llvm_order(success_order),
                        llvm_order(failure_order),
                    )
                    .map_err(|error| CodegenError(format!("atomic compare exchange: {error}")))?;
                let success = self
                    .builder
                    .build_extract_value(pair, 1, "atomic_success")
                    .map_err(|error| CodegenError(format!("atomic compare result: {error}")))?
                    .into_int_value();
                if location.kind == AtomicValueKind::Reference {
                    self.atomic_success_barrier(address, success)?;
                }
                let value = match result {
                    AtomicCompareExchangeResult::Success => success.into(),
                    AtomicCompareExchangeResult::ObservedValue => {
                        let old = self
                            .builder
                            .build_extract_value(pair, 0, "atomic_observed")
                            .map_err(|error| {
                                CodegenError(format!("atomic compare value: {error}"))
                            })?;
                        self.atomic_result(*location, old)?
                    }
                };
                self.temps.insert(*out, value);
            }
            _ => unreachable!("atomic instruction dispatch is exhaustive"),
        }
        Ok(())
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

fn llvm_order(order: AtomicMemoryOrder) -> AtomicOrdering {
    match order {
        AtomicMemoryOrder::Relaxed => AtomicOrdering::Monotonic,
        AtomicMemoryOrder::Acquire => AtomicOrdering::Acquire,
        AtomicMemoryOrder::Release => AtomicOrdering::Release,
        AtomicMemoryOrder::AcqRel => AtomicOrdering::AcquireRelease,
        AtomicMemoryOrder::SeqCst => AtomicOrdering::SequentiallyConsistent,
    }
}

fn llvm_rmw(operation: AtomicRmwOperation) -> AtomicRMWBinOp {
    match operation {
        AtomicRmwOperation::Exchange => AtomicRMWBinOp::Xchg,
        AtomicRmwOperation::Add => AtomicRMWBinOp::Add,
        AtomicRmwOperation::Subtract => AtomicRMWBinOp::Sub,
        AtomicRmwOperation::And => AtomicRMWBinOp::And,
        AtomicRmwOperation::Or => AtomicRMWBinOp::Or,
        AtomicRmwOperation::Xor => AtomicRMWBinOp::Xor,
    }
}
