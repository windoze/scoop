//! Darwin/AArch64 return-PC and managed frame-chain checks.

use std::fmt;

use crate::link_object::{
    BuiltinObjectSectionRoleV1, ParsedLlvmStackmapFunctionV3,
    ValidatedBuiltinObjectSectionInventoryV1, VerifiedStrongDefinitionSymbolV1,
    VerifiedStrongObjectDefinitionV1,
};

pub(super) fn validate_stackmap_function_machine_code(
    object_bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    definition: &VerifiedStrongObjectDefinitionV1,
    symbol: &VerifiedStrongDefinitionSymbolV1,
    function: &ParsedLlvmStackmapFunctionV3,
) -> Result<Vec<u64>, DarwinAarch64StackmapMachineCodeError> {
    let primary = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == definition.primary_atom())
        .copied()
        .ok_or(DarwinAarch64StackmapMachineCodeError::MissingPrimaryAtom)?;
    if primary.section_ordinal() != symbol.section_ordinal() || primary.start() != symbol.value() {
        return Err(DarwinAarch64StackmapMachineCodeError::PrimarySymbolMismatch);
    }
    let section_index = usize::from(primary.section_ordinal().get()) - 1;
    let section_role = sections
        .roles()
        .get(section_index)
        .copied()
        .ok_or(DarwinAarch64StackmapMachineCodeError::MissingTextSection)?;
    if section_role != BuiltinObjectSectionRoleV1::Text {
        return Err(
            DarwinAarch64StackmapMachineCodeError::FunctionOutsideTextSection {
                actual: section_role,
            },
        );
    }
    if symbol.value() % 4 != 0 {
        return Err(DarwinAarch64StackmapMachineCodeError::UnalignedFunction);
    }
    let return_pcs = function
        .records()
        .iter()
        .map(|record| validate_return_pc(object_bytes, sections, primary, symbol, record))
        .collect::<Result<Vec<_>, _>>()?;
    let first_return_pc = return_pcs
        .iter()
        .copied()
        .min()
        .ok_or(DarwinAarch64StackmapMachineCodeError::EmptyFunctionRecordSet)?;

    let mut pc = symbol.value();
    let mut frame_record = false;
    let mut frame_pointer = false;
    while pc < first_return_pc {
        let instruction = text_instruction(object_bytes, sections, primary, pc)?;
        frame_record |= is_frame_record_store(instruction);
        frame_pointer |= is_frame_pointer_setup(instruction);
        pc = pc
            .checked_add(4)
            .ok_or(DarwinAarch64StackmapMachineCodeError::AddressOverflow)?;
    }
    if !frame_record || !frame_pointer {
        return Err(
            DarwinAarch64StackmapMachineCodeError::MissingManagedFrameChain {
                frame_record,
                frame_pointer,
            },
        );
    }
    Ok(return_pcs)
}

fn validate_return_pc(
    object_bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    primary: crate::link_object::VerifiedDefinitionAtomRangeV1,
    symbol: &VerifiedStrongDefinitionSymbolV1,
    record: &crate::link_object::ParsedLlvmStackmapRecordV3,
) -> Result<u64, DarwinAarch64StackmapMachineCodeError> {
    if record.instruction_offset() % 4 != 0 {
        return Err(
            DarwinAarch64StackmapMachineCodeError::UnalignedInstructionOffset {
                safepoint_id: record.safepoint_id(),
                offset: record.instruction_offset(),
            },
        );
    }
    let return_pc = symbol
        .value()
        .checked_add(u64::from(record.instruction_offset()))
        .ok_or(DarwinAarch64StackmapMachineCodeError::AddressOverflow)?;
    let call_pc =
        return_pc
            .checked_sub(4)
            .ok_or(DarwinAarch64StackmapMachineCodeError::InvalidReturnPc {
                safepoint_id: record.safepoint_id(),
                return_pc,
            })?;
    if call_pc < primary.start() || return_pc > primary.end() {
        return Err(
            DarwinAarch64StackmapMachineCodeError::ReturnPcOutsideFunction {
                safepoint_id: record.safepoint_id(),
                return_pc,
            },
        );
    }
    let instruction = text_instruction(object_bytes, sections, primary, call_pc)?;
    if !is_call(instruction) {
        return Err(
            DarwinAarch64StackmapMachineCodeError::ReturnPcDoesNotFollowCall {
                safepoint_id: record.safepoint_id(),
                return_pc,
                instruction,
            },
        );
    }
    Ok(return_pc)
}

fn text_instruction(
    object_bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    primary: crate::link_object::VerifiedDefinitionAtomRangeV1,
    pc: u64,
) -> Result<u32, DarwinAarch64StackmapMachineCodeError> {
    let section_index = usize::from(primary.section_ordinal().get()) - 1;
    let section = sections
        .envelope()
        .sections()
        .get(section_index)
        .copied()
        .ok_or(DarwinAarch64StackmapMachineCodeError::MissingTextSection)?;
    let offset_in_section = pc
        .checked_sub(section.virtual_address())
        .ok_or(DarwinAarch64StackmapMachineCodeError::InstructionOutsideText)?;
    let file_offset = section
        .file_offset()
        .and_then(|start| start.checked_add(offset_in_section))
        .ok_or(DarwinAarch64StackmapMachineCodeError::InstructionOutsideText)?;
    let start = usize::try_from(file_offset)
        .map_err(|_| DarwinAarch64StackmapMachineCodeError::InstructionOutsideText)?;
    let end = start
        .checked_add(4)
        .ok_or(DarwinAarch64StackmapMachineCodeError::InstructionOutsideText)?;
    let bytes = object_bytes
        .get(start..end)
        .ok_or(DarwinAarch64StackmapMachineCodeError::InstructionOutsideText)?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte instruction"),
    ))
}

fn is_call(instruction: u32) -> bool {
    instruction & 0xfc00_0000 == 0x9400_0000 || instruction & 0xffff_fc1f == 0xd63f_0000
}

fn is_frame_record_store(instruction: u32) -> bool {
    let is_64_bit_pair = instruction >> 30 == 0b10;
    let is_pair_instruction = instruction >> 27 & 0b111 == 0b101;
    let is_integer_pair = instruction >> 26 & 1 == 0;
    let is_store = instruction >> 22 & 1 == 0;
    let first = instruction & 0x1f;
    let base = instruction >> 5 & 0x1f;
    let second = instruction >> 10 & 0x1f;
    is_64_bit_pair
        && is_pair_instruction
        && is_integer_pair
        && is_store
        && first == 29
        && second == 30
        && base == 31
}

fn is_frame_pointer_setup(instruction: u32) -> bool {
    instruction & 0xffc0_03ff == 0x9100_03fd
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DarwinAarch64StackmapMachineCodeError {
    MissingPrimaryAtom,
    PrimarySymbolMismatch,
    MissingTextSection,
    FunctionOutsideTextSection {
        actual: BuiltinObjectSectionRoleV1,
    },
    UnalignedFunction,
    EmptyFunctionRecordSet,
    UnalignedInstructionOffset {
        safepoint_id: u64,
        offset: u32,
    },
    InvalidReturnPc {
        safepoint_id: u64,
        return_pc: u64,
    },
    ReturnPcOutsideFunction {
        safepoint_id: u64,
        return_pc: u64,
    },
    ReturnPcDoesNotFollowCall {
        safepoint_id: u64,
        return_pc: u64,
        instruction: u32,
    },
    InstructionOutsideText,
    AddressOverflow,
    MissingManagedFrameChain {
        frame_record: bool,
        frame_pointer: bool,
    },
}

impl fmt::Display for DarwinAarch64StackmapMachineCodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid managed AArch64 machine code: {self:?}")
    }
}

impl std::error::Error for DarwinAarch64StackmapMachineCodeError {}
