use super::*;

mod addresses;
mod aggregates;
mod arrays;
mod callbacks;
mod enums;
mod exceptions;
mod heap;
mod operators;
mod values;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn instruction(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        match instruction {
            Instruction::BinOp { .. }
            | Instruction::UnaryOp { .. }
            | Instruction::IntegerUnary { .. }
            | Instruction::IntegerBinary { .. }
            | Instruction::SafeIntegerDivRem { .. }
            | Instruction::IntegerCompare { .. }
            | Instruction::IntegerCompareTo { .. }
            | Instruction::IntegerShift { .. }
            | Instruction::IntegerConvert { .. } => self.emit_operator_instruction(instruction),
            Instruction::MakeAggregate { .. } | Instruction::ExtractValue { .. } => {
                self.emit_aggregate_instruction(instruction)
            }
            Instruction::HeapLoad { .. }
            | Instruction::MachineHeapLoad { .. }
            | Instruction::AtomicLoad { .. }
            | Instruction::HeapStore { .. }
            | Instruction::MachineHeapStore { .. }
            | Instruction::AtomicStore { .. }
            | Instruction::AtomicCompareExchange { .. } => self.emit_heap_instruction(instruction),
            Instruction::FunctionAddress { .. }
            | Instruction::ForeignCallbackRegister { .. }
            | Instruction::ForeignCallbackOperation(_) => {
                self.emit_callback_instruction(instruction)
            }
            Instruction::ULongToPtr { .. }
            | Instruction::PtrToULong { .. }
            | Instruction::RawLoad { .. }
            | Instruction::RawStore { .. }
            | Instruction::PtrOffset { .. }
            | Instruction::LocalAddress { .. }
            | Instruction::GlobalLoad { .. }
            | Instruction::GlobalStore { .. }
            | Instruction::GlobalAddress { .. }
            | Instruction::NativeGlobalLoad { .. }
            | Instruction::NativeGlobalStore { .. }
            | Instruction::NativeGlobalAddress { .. }
            | Instruction::Store { .. } => self.emit_address_instruction(instruction),
            Instruction::Call { site } => self.emit_call_site(site),
            Instruction::ManagedPoll { site } => self.safepoint_poll(site),
            Instruction::Invoke { site } => self.emit_invoke_site(site),
            Instruction::LandingPad { .. }
            | Instruction::CleanupPad { .. }
            | Instruction::BeginCatch { .. }
            | Instruction::EndCatch
            | Instruction::Throw { .. } => self.emit_exception_instruction(instruction),
            Instruction::ArrayAlloc { .. }
            | Instruction::ArrayAssembly { .. }
            | Instruction::ArrayLen { .. }
            | Instruction::ArrayGet { .. }
            | Instruction::ArraySet { .. }
            | Instruction::ArrayClone { .. } => self.emit_array_instruction(instruction),
            Instruction::EnumWrap { .. }
            | Instruction::EnumTag { .. }
            | Instruction::EnumField { .. } => self.emit_enum_instruction(instruction),
        }
    }
}
