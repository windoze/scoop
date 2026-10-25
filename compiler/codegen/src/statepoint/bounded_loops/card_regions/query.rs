//! Recognize the runtime's immutable page-map path and region prefix.

use inkwell::{AtomicOrdering, types::BasicTypeEnum, values::PointerValue};

use super::super::operations::{address, ordinary};
use super::InstructionValue;
use crate::statepoint::{BasicValue, InstructionOpcode};

pub(super) fn immutable_load(instruction: InstructionValue<'_>) -> bool {
    if instruction.get_opcode() != InstructionOpcode::Load {
        return false;
    }
    if page_load(instruction) {
        return true;
    }
    ordinary(instruction)
        && address(instruction, 0).is_some_and(|pointer| {
            match (prefix_offset(pointer), instruction.get_type()) {
                (Some(0), inkwell::types::AnyTypeEnum::IntType(int)) => int.get_bit_width() == 64,
                (Some(16), inkwell::types::AnyTypeEnum::PointerType(pointer)) => {
                    pointer.get_address_space() == inkwell::AddressSpace::default()
                }
                _ => false,
            }
        })
}

pub(super) fn page_load(instruction: InstructionValue<'_>) -> bool {
    PointerValue::try_from(instruction)
        .ok()
        .and_then(|pointer| page_level(pointer, 4))
        .is_some_and(|depth| depth > 0)
}

fn page_level(pointer: PointerValue<'_>, remaining: u8) -> Option<u8> {
    if pointer.get_type().get_address_space() != inkwell::AddressSpace::default() {
        return None;
    }
    if pointer.is_const()
        && pointer.get_name().to_bytes()
            == scoop_lir::RuntimeAbiSymbolV1::PageMap
                .logical_symbol()
                .as_bytes()
    {
        return Some(0);
    }
    if remaining == 0 {
        return None;
    }
    let load = pointer.as_instruction_value()?;
    if load.get_opcode() != InstructionOpcode::Load
        || load.get_volatile() != Ok(false)
        || load.get_atomic_ordering() != Ok(AtomicOrdering::Acquire)
    {
        return None;
    }
    let slot = address(load, 0)?;
    let base = match slot.as_instruction_value() {
        Some(gep) if gep.get_opcode() == InstructionOpcode::GetElementPtr => {
            match gep.get_gep_source_element_type().ok()? {
                BasicTypeEnum::PointerType(_) => {}
                BasicTypeEnum::ArrayType(array)
                    if array.len() == 4096 && array.get_element_type().is_pointer_type() => {}
                _ => return None,
            }
            address(gep, 0)?
        }
        _ => slot,
    };
    page_level(base, remaining - 1).map(|depth| depth + 1)
}

fn prefix_offset(pointer: PointerValue<'_>) -> Option<u64> {
    if page_level(pointer, 4) == Some(4) {
        return Some(0);
    }
    let gep = pointer.as_instruction_value()?;
    if gep.get_opcode() != InstructionOpcode::GetElementPtr
        || page_level(address(gep, 0)?, 4) != Some(4)
    {
        return None;
    }
    let offset = match gep.get_gep_source_element_type().ok()? {
        BasicTypeEnum::IntType(int) if gep.get_num_operands() == 2 => {
            integer(gep, 1)?.checked_mul(u64::from(int.get_bit_width() / 8))?
        }
        BasicTypeEnum::StructType(structure)
            if gep.get_num_operands() == 3
                && integer(gep, 1) == Some(0)
                && !structure.is_packed()
                && structure.count_fields() == 3 =>
        {
            let fields = structure.get_field_types();
            if !fields[..2]
                .iter()
                .all(|ty| ty.is_int_type() && ty.into_int_type().get_bit_width() == 64)
                || !fields[2].is_pointer_type()
            {
                return None;
            }
            integer(gep, 2)?.checked_mul(8)?
        }
        _ => return None,
    };
    matches!(offset, 0 | 16).then_some(offset)
}

pub(super) fn card_store(instruction: InstructionValue<'_>) -> bool {
    if instruction.get_opcode() != InstructionOpcode::Store
        || instruction.get_volatile() != Ok(false)
        || instruction.get_atomic_ordering() != Ok(AtomicOrdering::Monotonic)
        || integer(instruction, 0) != Some(1)
    {
        return false;
    }
    let Some(gep) = address(instruction, 1).and_then(|p| p.as_instruction_value()) else {
        return false;
    };
    if gep.get_opcode() != InstructionOpcode::GetElementPtr {
        return false;
    }
    let Some(cards) = address(gep, 0).and_then(|p| p.as_instruction_value()) else {
        return false;
    };
    cards.get_opcode() == InstructionOpcode::Load
        && ordinary(cards)
        && address(cards, 0).is_some_and(|pointer| prefix_offset(pointer) == Some(16))
}

pub(super) fn cards_region(cards: PointerValue<'_>) -> Option<PointerValue<'_>> {
    let load = cards.as_instruction_value()?;
    if load.get_opcode() != InstructionOpcode::Load || !ordinary(load) {
        return None;
    }
    let slot = address(load, 0)?;
    if prefix_offset(slot) != Some(16) {
        return None;
    }
    address(slot.as_instruction_value()?, 0)
}

fn integer(instruction: InstructionValue<'_>, index: u32) -> Option<u64> {
    let value = instruction.get_operand(index)?.value()?;
    value
        .is_int_value()
        .then(|| value.into_int_value().get_zero_extended_constant())?
}
