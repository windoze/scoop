use super::*;

mod addresses;
mod aggregates;
mod arrays;
mod atomics;
mod boxing;
mod callbacks;
mod data_borrow;
mod enums;
mod exceptions;
mod floating;
mod heap;
mod operators;
mod pointers;
mod values;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn instruction(&mut self, instruction: &Instruction) -> Result<(), CodegenError> {
        match instruction {
            Instruction::AtomicLoad { .. }
            | Instruction::AtomicStore { .. }
            | Instruction::AtomicRmw { .. }
            | Instruction::AtomicCmpXchg { .. } => self.emit_atomic_instruction(instruction),
            Instruction::PushPinFrame { .. }
            | Instruction::PopPinFrame { .. }
            | Instruction::ArrayDataPointer { .. }
            | Instruction::StringDataPointer { .. }
            | Instruction::BorrowDataLength { .. } => self.emit_data_borrow(instruction),
            Instruction::BoxValue { .. } | Instruction::UnboxValue { .. } => {
                self.emit_boxing(instruction)
            }
            Instruction::FloatUnary { .. }
            | Instruction::FloatBinary { .. }
            | Instruction::FloatConversion { .. } => self.emit_float_instruction(instruction),
            Instruction::BinOp { .. }
            | Instruction::UnaryOp { .. }
            | Instruction::IntegerUnary { .. }
            | Instruction::IntegerBinary { .. }
            | Instruction::SafeIntegerDivRem { .. }
            | Instruction::IntegerCompare { .. }
            | Instruction::IntegerCompareTo { .. }
            | Instruction::IntegerShift { .. }
            | Instruction::IntegerConvert { .. } => self.emit_operator_instruction(instruction),
            Instruction::MakeAggregate { .. }
            | Instruction::MakeZstValue { .. }
            | Instruction::ExtractValue { .. } => self.emit_aggregate_instruction(instruction),
            Instruction::HeapLoad { .. }
            | Instruction::ReleaseFieldLoad { .. }
            | Instruction::PublishReleaseReady { .. }
            | Instruction::MachineHeapLoad { .. }
            | Instruction::MachineAtomicLoad { .. }
            | Instruction::HeapStore { .. }
            | Instruction::MachineHeapStore { .. }
            | Instruction::MachineAtomicStore { .. }
            | Instruction::MachineAtomicCompareExchange { .. } => {
                self.emit_heap_instruction(instruction)
            }
            Instruction::FunctionAddress { .. }
            | Instruction::ForeignCallbackRegister { .. }
            | Instruction::ForeignCallbackOperation(_) => {
                self.emit_callback_instruction(instruction)
            }
            Instruction::ULongToPtr { .. }
            | Instruction::PtrToULong { .. }
            | Instruction::LocalAddress { .. }
            | Instruction::GlobalLoad { .. }
            | Instruction::GlobalStore { .. }
            | Instruction::GlobalAddress { .. }
            | Instruction::NativeGlobalLoad { .. }
            | Instruction::NativeGlobalStore { .. }
            | Instruction::NativeGlobalAddress { .. }
            | Instruction::Store { .. } => self.emit_address_instruction(instruction),
            Instruction::RawLoad { .. }
            | Instruction::RawStore { .. }
            | Instruction::PtrOffset { .. } => self.emit_raw_pointer_instruction(instruction),
            Instruction::Call { site } => self.emit_call_site(site),
            Instruction::ManagedPoll { site } => self.safepoint_poll(site),
            Instruction::Invoke { site } => self.emit_invoke_site(site),
            Instruction::LandingPad { .. }
            | Instruction::CleanupPad { .. }
            | Instruction::BeginCatch { .. }
            | Instruction::EndCatch
            | Instruction::Throw { .. } => self.emit_exception_instruction(instruction),
            Instruction::ArrayAllocDynamic { .. }
            | Instruction::ArrayAlloc { .. }
            | Instruction::ArrayAssembly { .. }
            | Instruction::ArrayLen { .. }
            | Instruction::ArrayGet { .. }
            | Instruction::ArraySet { .. }
            | Instruction::ArrayClone { .. } => self.emit_array_instruction(instruction),
            Instruction::EnumWrap { .. }
            | Instruction::EnumTag { .. }
            | Instruction::EnumField { .. }
            | Instruction::VariantTest { .. }
            | Instruction::VariantPayloadProject { .. } => self.emit_enum_instruction(instruction),
        }
    }
}
