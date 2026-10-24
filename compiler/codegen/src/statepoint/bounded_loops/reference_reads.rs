//! Partially unroll small read-only reference kernels without extending roots.

use inkwell::AddressSpace;

use super::{InstructionOpcode, PointerValue, operations, region::Region};

pub(super) const FACTOR: u64 = 4;

pub(super) fn selected(region: &Region<'_>) -> bool {
    if region.blocks.len() != 1 || region.iterations < FACTOR || region.iterations % FACTOR != 0 {
        return false;
    }
    let mut reference_read = false;
    let mut size = 0;
    for instruction in region.header.get_instructions() {
        if !matches!(
            instruction.get_opcode(),
            InstructionOpcode::Phi | InstructionOpcode::Br
        ) {
            size += 1;
            if size > 24 {
                return false;
            }
        }
        let supported = match instruction.get_opcode() {
            InstructionOpcode::Br => true,
            InstructionOpcode::Store => false,
            InstructionOpcode::GetElementPtr => {
                PointerValue::try_from(instruction).is_ok_and(managed)
                    && operations::only_used_in(instruction, &region.blocks)
                    && instruction.get_operands().skip(1).all(|operand| {
                        operand.is_some_and(|operand| {
                            operand.value().is_some_and(|value| value.is_int_value())
                        })
                    })
            }
            InstructionOpcode::Load => {
                if operations::root_load(instruction, &region.blocks) {
                    continue;
                }
                let reference = PointerValue::try_from(instruction).is_ok_and(managed);
                reference_read |= reference;
                operations::ordinary(instruction)
                    && operations::address(instruction, 0).is_some_and(managed)
                    && (instruction.get_type().is_int_type()
                        || instruction.get_type().is_float_type()
                        || (reference && operations::only_used_in(instruction, &region.blocks)))
            }
            _ => operations::supported(instruction, &region.blocks, &[]),
        };
        if !supported {
            return false;
        }
    }
    reference_read
}

fn managed(pointer: PointerValue<'_>) -> bool {
    pointer.get_type().get_address_space() == AddressSpace::from(1)
}
