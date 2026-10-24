//! Stack reference reloads stay in place while independent GC queries move.

use std::collections::HashSet;

use inkwell::values::{BasicValue, InstructionOpcode, InstructionValue, PhiValue, PointerValue};
use inkwell::{AddressSpace, AtomicOrdering};

use super::super::operations::address;

pub(super) fn retained_load(instruction: InstructionValue<'_>) -> bool {
    if instruction.get_volatile() != Ok(true)
        || instruction.get_atomic_ordering() != Ok(AtomicOrdering::NotAtomic)
        || !PointerValue::try_from(instruction)
            .is_ok_and(|value| value.get_type().get_address_space() == AddressSpace::from(1))
    {
        return false;
    }
    let Some(pointer) = address(instruction, 0) else {
        return false;
    };
    let entry = instruction
        .get_parent()
        .and_then(|block| block.get_parent())
        .and_then(|function| function.get_first_basic_block());
    let mut pending = vec![pointer];
    let mut visited = HashSet::new();
    let mut has_storage = false;
    while let Some(pointer) = pending.pop() {
        if pointer.get_type().get_address_space() != AddressSpace::default() {
            return false;
        }
        let Some(definition) = pointer.as_instruction_value() else {
            return false;
        };
        if !visited.insert(definition) {
            continue;
        }
        match definition.get_opcode() {
            InstructionOpcode::Alloca if definition.get_parent() == entry => has_storage = true,
            InstructionOpcode::GetElementPtr
                if definition.get_operands().skip(1).all(|operand| {
                    operand
                        .and_then(|value| value.value())
                        .is_some_and(|value| {
                            value.is_int_value()
                                && value
                                    .into_int_value()
                                    .get_zero_extended_constant()
                                    .is_some()
                        })
                }) =>
            {
                pending.push(address(definition, 0).expect("GEP has a pointer base"));
            }
            InstructionOpcode::Select => {
                pending.push(address(definition, 1).expect("pointer select has pointer operands"));
                pending.push(address(definition, 2).expect("pointer select has pointer operands"));
            }
            InstructionOpcode::Phi => {
                let phi = PhiValue::try_from(definition).expect("pointer phi");
                pending.extend(
                    phi.get_incomings()
                        .map(|(value, _)| value.into_pointer_value()),
                );
            }
            _ => return false,
        }
    }
    has_storage
}
