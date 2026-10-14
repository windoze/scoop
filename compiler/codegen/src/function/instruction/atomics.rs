use super::*;
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
                self.heap_atomic_store(
                    address,
                    value,
                    *location,
                    llvm_order(order.memory_order()),
                )?;
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
                let old = self.heap_atomic_rmw(
                    address,
                    value,
                    *location,
                    llvm_rmw(*operation),
                    llvm_order(*order),
                )?;
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
                let (pair, success) = self.heap_atomic_cmpxchg(
                    address,
                    expected,
                    replacement,
                    *location,
                    (llvm_order(success_order), llvm_order(failure_order)),
                )?;
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
