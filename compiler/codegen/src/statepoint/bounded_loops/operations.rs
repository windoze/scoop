//! Keep vectorization inside the bounded loop and away from GC root storage.

use inkwell::values::{AnyValueEnum, BasicValueEnum};
use inkwell::{AddressSpace, AtomicOrdering};

use super::{
    BasicBlock, BasicValue, InstructionOpcode, InstructionValue, PointerValue, region::Region,
};

pub(super) fn prepare(region: &Region<'_>) -> bool {
    let loads = region
        .blocks
        .iter()
        .flat_map(|block| block.get_instructions())
        .filter(|instruction| root_load(*instruction, &region.blocks))
        .collect::<Vec<_>>();
    if !region
        .blocks
        .iter()
        .flat_map(|block| block.get_instructions())
        .all(|instruction| {
            instruction.get_opcode() == InstructionOpcode::Br
                || loads.contains(&instruction)
                || supported(instruction, &region.blocks, &loads)
        })
    {
        return false;
    }
    if !loads.is_empty() {
        let builder = region.header.get_context().create_builder();
        builder.position_before(
            &region
                .predecessor
                .get_terminator()
                .expect("loop predecessor terminates"),
        );
        for instruction in loads {
            instruction.remove_from_basic_block();
            builder.insert_instruction(&instruction, None);
        }
    }
    true
}

fn supported(
    instruction: InstructionValue<'_>,
    blocks: &[BasicBlock<'_>],
    loads: &[InstructionValue<'_>],
) -> bool {
    match instruction.get_opcode() {
        InstructionOpcode::GetElementPtr => {
            only_used_in(instruction, blocks)
                && PointerValue::try_from(instruction)
                    .is_ok_and(|pointer| derived(pointer, blocks, loads))
        }
        InstructionOpcode::Load => {
            (instruction.get_type().is_int_type() || instruction.get_type().is_float_type())
                && ordinary(instruction)
                && address(instruction, 0).is_some_and(|pointer| derived(pointer, blocks, loads))
        }
        InstructionOpcode::Store => {
            instruction
                .get_operand(0)
                .and_then(|operand| operand.value())
                .is_some_and(scalar)
                && ordinary(instruction)
                && address(instruction, 1).is_some_and(|pointer| derived(pointer, blocks, loads))
        }
        InstructionOpcode::Phi
        | InstructionOpcode::Add
        | InstructionOpcode::Sub
        | InstructionOpcode::Mul
        | InstructionOpcode::And
        | InstructionOpcode::Or
        | InstructionOpcode::Xor
        | InstructionOpcode::Shl
        | InstructionOpcode::LShr
        | InstructionOpcode::AShr
        | InstructionOpcode::Trunc
        | InstructionOpcode::SExt
        | InstructionOpcode::ZExt
        | InstructionOpcode::ICmp
        | InstructionOpcode::FAdd
        | InstructionOpcode::FSub
        | InstructionOpcode::FMul
        | InstructionOpcode::FDiv
        | InstructionOpcode::FRem
        | InstructionOpcode::FNeg
        | InstructionOpcode::FCmp
        | InstructionOpcode::SIToFP
        | InstructionOpcode::UIToFP
        | InstructionOpcode::FPTrunc
        | InstructionOpcode::FPExt
        | InstructionOpcode::BitCast
        | InstructionOpcode::Select => {
            (instruction.get_type().is_int_type() || instruction.get_type().is_float_type())
                && instruction.get_operands().all(|operand| {
                    operand.is_some_and(|operand| operand.value().is_some_and(scalar))
                })
        }
        _ => false,
    }
}

fn scalar(value: BasicValueEnum<'_>) -> bool {
    value.is_int_value() || value.is_float_value()
}

pub(super) fn ordinary(instruction: InstructionValue<'_>) -> bool {
    instruction.get_volatile() == Ok(false)
        && instruction.get_atomic_ordering() == Ok(AtomicOrdering::NotAtomic)
}

pub(super) fn address(instruction: InstructionValue<'_>, index: u32) -> Option<PointerValue<'_>> {
    let value = instruction.get_operand(index)?.value()?;
    value.is_pointer_value().then(|| value.into_pointer_value())
}

fn derived(
    mut pointer: PointerValue<'_>,
    blocks: &[BasicBlock<'_>],
    loads: &[InstructionValue<'_>],
) -> bool {
    loop {
        if pointer.get_type().get_address_space() != AddressSpace::from(1) {
            return false;
        }
        let Some(instruction) = pointer.as_instruction_value() else {
            return true;
        };
        if !instruction
            .get_parent()
            .is_some_and(|parent| blocks.contains(&parent))
            || loads.contains(&instruction)
        {
            return true;
        }
        if instruction.get_opcode() != InstructionOpcode::GetElementPtr
            || !instruction.get_operands().skip(1).all(|operand| {
                operand.is_some_and(|operand| {
                    operand.value().is_some_and(|value| value.is_int_value())
                })
            })
        {
            return false;
        }
        let Some(base) = address(instruction, 0) else {
            return false;
        };
        pointer = base;
    }
}

pub(super) fn root_load(instruction: InstructionValue<'_>, blocks: &[BasicBlock<'_>]) -> bool {
    if instruction.get_opcode() != InstructionOpcode::Load
        || !ordinary(instruction)
        || !PointerValue::try_from(instruction)
            .is_ok_and(|value| value.get_type().get_address_space() == AddressSpace::from(1))
        || !only_used_in(instruction, blocks)
    {
        return false;
    }
    let Some(slot) = address(instruction, 0).and_then(|value| value.as_instruction_value()) else {
        return false;
    };
    if slot.get_opcode() != InstructionOpcode::Alloca
        || slot.get_parent()
            != instruction
                .get_parent()
                .and_then(|block| block.get_parent())
                .and_then(|function| function.get_first_basic_block())
    {
        return false;
    }
    let slot_value = PointerValue::try_from(slot).expect("alloca produces a pointer");
    let mut next = slot.get_first_use();
    while let Some(usage) = next {
        next = usage.get_next_use();
        let Some(user) = user_instruction(usage.get_user()) else {
            return false;
        };
        match user.get_opcode() {
            InstructionOpcode::Load if address(user, 0) == Some(slot_value) => {}
            InstructionOpcode::Store
                if address(user, 1) == Some(slot_value)
                    && address(user, 0) != Some(slot_value)
                    && !user
                        .get_parent()
                        .is_some_and(|parent| blocks.contains(&parent)) => {}
            _ => return false,
        }
    }
    true
}

fn only_used_in(instruction: InstructionValue<'_>, blocks: &[BasicBlock<'_>]) -> bool {
    let mut next = instruction.get_first_use();
    while let Some(usage) = next {
        next = usage.get_next_use();
        if !user_instruction(usage.get_user())
            .and_then(|user| user.get_parent())
            .is_some_and(|parent| blocks.contains(&parent))
        {
            return false;
        }
    }
    true
}

fn user_instruction(value: AnyValueEnum<'_>) -> Option<InstructionValue<'_>> {
    match value {
        AnyValueEnum::InstructionValue(value) => Some(value),
        _ => BasicValueEnum::try_from(value).ok()?.as_instruction_value(),
    }
}
