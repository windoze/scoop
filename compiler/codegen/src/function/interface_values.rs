//! Reuse freshly stored interface snapshots without crossing GC or alias effects.

use inkwell::basic_block::BasicBlock;

use super::*;

pub(super) struct StoredInterfaces<'ctx> {
    enabled: bool,
    block: Option<BasicBlock<'ctx>>,
    values: HashMap<PointerValue<'ctx>, BasicValueEnum<'ctx>>,
}

impl<'ctx> StoredInterfaces<'ctx> {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            block: None,
            values: HashMap::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.block = None;
    }

    pub fn retain(&mut self, instruction: &Instruction) {
        if self.values.is_empty() {
            return;
        }
        // All calls, polls, allocations, address escapes, raw memory, atomics
        // and future instructions end the segment. Typed local writes update
        // their own snapshots; typed heap accesses cannot alias stack storage.
        if !matches!(
            instruction,
            Instruction::Store { .. }
                | Instruction::IntegerUnary { .. }
                | Instruction::IntegerBinary { .. }
                | Instruction::IntegerCompare { .. }
                | Instruction::IntegerCompareTo { .. }
                | Instruction::IntegerShift { .. }
                | Instruction::IntegerConvert { .. }
                | Instruction::FloatUnary { .. }
                | Instruction::FloatBinary { .. }
                | Instruction::FloatConversion { .. }
                | Instruction::MakeAggregate { .. }
                | Instruction::MakeZstValue { .. }
                | Instruction::ExtractValue { .. }
                | Instruction::PtrOffset { .. }
                | Instruction::ArrayLen { .. }
                | Instruction::ArrayGet { .. }
                | Instruction::ArraySet { .. }
                | Instruction::HeapLoad { .. }
                | Instruction::HeapStore { .. }
                | Instruction::EnumWrap { .. }
                | Instruction::EnumTag { .. }
                | Instruction::EnumField { .. }
                | Instruction::VariantTest { .. }
                | Instruction::VariantPayloadProject { .. }
        ) {
            self.clear();
        }
    }

    pub fn get(
        &self,
        block: BasicBlock<'ctx>,
        storage: PointerValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        (self.block == Some(block))
            .then(|| self.values.get(&storage).copied())
            .flatten()
    }

    fn insert(
        &mut self,
        block: BasicBlock<'ctx>,
        storage: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        if !self.enabled {
            return;
        }
        if self.block != Some(block) {
            self.clear();
            self.block = Some(block);
        }
        self.values.insert(storage, value);
    }
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn remember_interface_value(
        &mut self,
        ty: &LirType,
        storage: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        if self.is_interface_storage(ty) {
            let block = self
                .builder
                .get_insert_block()
                .expect("interface store block");
            self.interface_values.insert(block, storage, value);
        }
    }
}
