//! Finish an unconditional strided write stream before the next outer poll.

use inkwell::AtomicOrdering;
use inkwell::builder::Builder;
use inkwell::values::{IntValue, PointerValue};

use super::*;
use crate::CARD_SHIFT;

mod offsets;

struct Batch<'ctx> {
    mark: InstructionValue<'ctx>,
    slot: InstructionValue<'ctx>,
    index: InstructionValue<'ctx>,
    cards: PointerValue<'ctx>,
    record: crate::gc_card_groups::Record<'ctx>,
    span: u64,
}

pub(super) fn coalesce<'ctx>(
    region: &mut Region<'ctx>,
    machine: &TargetMachine,
    mandatory: &HashSet<BasicBlock<'ctx>>,
) -> Result<(), CodegenError> {
    if region.iterations == 1 {
        return Ok(());
    }
    let mut offsets = offsets::Offsets::new(region, machine.get_target_data());
    let mut records = region
        .blocks
        .iter()
        .flat_map(|block| block.get_instructions())
        .filter_map(crate::gc_card_groups::record)
        .filter(|record| {
            record
                .instruction
                .get_parent()
                .is_some_and(|block| mandatory.contains(&block))
        })
        .collect::<Vec<_>>();
    let batches = region
        .blocks
        .iter()
        .flat_map(|block| block.get_instructions())
        .filter(|instruction| query::card_store(*instruction))
        .filter_map(|instruction| {
            Batch::analyze(instruction, region, mandatory, &mut offsets, &mut records)
        })
        .collect::<Vec<_>>();
    if batches.is_empty() {
        return Ok(());
    }
    let exit = edges::split(region.latch, region.exit, "card.exit")?;
    region.exit = exit;
    let builder = region.header.get_context().create_builder();
    builder.position_before(&exit.get_terminator().expect("card exit terminates"));
    for batch in &batches {
        batch.emit(&builder)?;
        batch.record.instruction.erase_from_basic_block();
        batch.mark.erase_from_basic_block();
    }
    // Remove only the old address tail. Other stores can still share a query.
    let mut visited = HashSet::new();
    for instruction in batches.iter().map(|batch| batch.slot) {
        if visited.insert(instruction) && instruction.get_first_use().is_none() {
            instruction.erase_from_basic_block();
        }
    }
    for batch in batches {
        if !visited.insert(batch.index) {
            continue;
        }
        let offset = batch.offset();
        if batch.index.get_first_use().is_none() {
            batch.index.erase_from_basic_block();
        }
        if let Some(offset) = offset.as_instruction_value() {
            sink(offset, region, exit, &builder);
        }
    }
    Ok(())
}

impl<'ctx> Batch<'ctx> {
    fn analyze(
        mark: InstructionValue<'ctx>,
        region: &Region<'ctx>,
        mandatory: &HashSet<BasicBlock<'ctx>>,
        offsets: &mut offsets::Offsets<'_, 'ctx>,
        records: &mut Vec<crate::gc_card_groups::Record<'ctx>>,
    ) -> Option<Self> {
        if !mark
            .get_parent()
            .is_some_and(|block| mandatory.contains(&block))
        {
            return None;
        }
        let slot = operations::address(mark, 1)?.as_instruction_value()?;
        let element = slot.get_gep_source_element_type().ok()?;
        if slot.get_num_operands() != 2
            || !element.is_int_type()
            || element.into_int_type().get_bit_width() != 8
        {
            return None;
        }
        let cards = operations::address(slot, 0)?;
        if cards
            .as_instruction_value()
            .and_then(|value| value.get_parent())
            .is_some_and(|block| region.blocks.contains(&block))
        {
            return None;
        }
        let index = slot.get_operand(1)?.value()?.as_instruction_value()?;
        if index.get_opcode() != InstructionOpcode::LShr
            || index
                .get_operand(1)?
                .value()?
                .into_int_value()
                .get_zero_extended_constant()
                != Some(CARD_SHIFT)
        {
            return None;
        }
        let offset = index.get_operand(0)?.value()?.into_int_value();
        let stride = offsets.stride(offset)?;
        if !matches!(stride, 0 | 8 | 16) {
            return None;
        }
        let mapping = query::cards_region(cards)?;
        let position = records.iter().position(|record| {
            record.region == mapping && record.card.as_instruction_value() == Some(index)
        })?;
        Some(Self {
            mark,
            slot,
            index,
            cards,
            record: records.swap_remove(position),
            span: stride * (region.iterations - 1),
        })
    }

    fn offset(&self) -> IntValue<'ctx> {
        // Splitting the exit replaces outer phis. Read their updated uses,
        // rather than retaining a value handle across that replacement.
        self.index
            .get_operand(0)
            .expect("card index has an offset")
            .value()
            .expect("card offset is a value")
            .into_int_value()
    }

    fn emit(&self, builder: &Builder<'ctx>) -> Result<(), CodegenError> {
        let offset = self.offset();
        let ty = offset.get_type();
        let start = builder
            .build_int_sub(offset, ty.const_int(self.span, false), "card.first_offset")
            .map_err(error)?;
        let first = builder
            .build_right_shift(start, ty.const_int(CARD_SHIFT, false), false, "card.first")
            .map_err(error)?;
        for index in 0..=(self.span >> CARD_SHIFT) {
            let card = if index == 0 {
                first
            } else {
                builder
                    .build_int_add(first, ty.const_int(index, false), "card.middle")
                    .map_err(error)?
            };
            self.mark(builder, card)?;
        }
        if self.span & ((1 << CARD_SHIFT) - 1) != 0 {
            let last = builder
                .build_right_shift(offset, ty.const_int(CARD_SHIFT, false), false, "card.last")
                .map_err(error)?;
            self.mark(builder, last)?;
        }
        Ok(())
    }

    fn mark(&self, builder: &Builder<'ctx>, index: IntValue<'ctx>) -> Result<(), CodegenError> {
        let byte = self.cards.get_type().get_context().i8_type();
        // SAFETY: every selected index is a card touched by this completed,
        // constant-stride write stream in its unchanged heap mapping.
        let slot = unsafe { builder.build_gep(byte, self.cards, &[index], "card.batch") }
            .map_err(error)?;
        builder
            .build_store(slot, byte.const_int(1, false))
            .map_err(error)?
            .set_atomic_ordering(AtomicOrdering::Monotonic)
            .map_err(|error| CodegenError(format!("batched card store: {error}")))?;
        builder
            .build_call(
                self.record.callee,
                &[self.record.region.into(), index.into()],
                "",
            )
            .map_err(error)?;
        Ok(())
    }
}

fn sink<'ctx>(
    instruction: InstructionValue<'ctx>,
    region: &Region<'ctx>,
    exit: BasicBlock<'ctx>,
    builder: &Builder<'ctx>,
) {
    if !pure_address(instruction)
        || !instruction
            .get_parent()
            .is_some_and(|block| region.blocks.contains(&block))
    {
        return;
    }
    let mut users = HashSet::new();
    let mut next = instruction.get_first_use();
    while let Some(usage) = next {
        next = usage.get_next_use();
        let Some(user) = operations::user_instruction(usage.get_user()) else {
            return;
        };
        if user
            .get_parent()
            .is_some_and(|block| region.blocks.contains(&block))
        {
            return;
        }
        users.insert(user);
    }
    let anchor = exit
        .get_instructions()
        .find(|user| users.contains(user))
        .unwrap_or_else(|| exit.get_terminator().expect("card exit terminates"));
    instruction.remove_from_basic_block();
    builder.position_before(&anchor);
    builder.insert_instruction(&instruction, None);
    for operand in instruction
        .get_operands()
        .filter_map(|operand| operand.and_then(|operand| operand.value()))
    {
        if let Some(definition) = operand.as_instruction_value() {
            sink(definition, region, exit, builder);
        }
    }
}

fn error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("card batch: {error}"))
}
