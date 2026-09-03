//! Darwin/AArch64 instruction and frame-chain verification.

use crate::CodegenError;

use super::{FunctionRelocation, TextSection};

fn text_instruction(text: &TextSection, pc: u64, what: &str) -> Result<u32, CodegenError> {
    let offset = pc.checked_sub(text.address).ok_or_else(|| {
        CodegenError(format!(
            "AArch64 {what} address {pc:#x} precedes __text at {:#x}",
            text.address
        ))
    })?;
    let offset = usize::try_from(offset)
        .map_err(|_| CodegenError(format!("AArch64 {what} offset exceeds usize::MAX")))?;
    let end = offset.checked_add(4).ok_or_else(|| {
        CodegenError(format!(
            "AArch64 {what} instruction range overflows usize::MAX"
        ))
    })?;
    let bytes = text.bytes.get(offset..end).ok_or_else(|| {
        CodegenError(format!(
            "AArch64 {what} instruction at {pc:#x} lies outside __text"
        ))
    })?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte instruction"),
    ))
}

fn is_aarch64_call(instruction: u32) -> bool {
    instruction & 0xfc00_0000 == 0x9400_0000 || instruction & 0xffff_fc1f == 0xd63f_0000
}

fn is_aarch64_frame_record_store(instruction: u32) -> bool {
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

fn is_aarch64_frame_pointer_setup(instruction: u32) -> bool {
    instruction & 0xffc0_03ff == 0x9100_03fd
}

pub(super) fn validate_aarch64_frame_chain(
    text: &TextSection,
    function: &FunctionRelocation,
    first_return_pc: u64,
) -> Result<(), CodegenError> {
    let mut pc = function.address;
    let mut frame_record = false;
    let mut frame_pointer = false;
    while pc < first_return_pc {
        let instruction = text_instruction(text, pc, "function prologue")?;
        frame_record |= is_aarch64_frame_record_store(instruction);
        frame_pointer |= is_aarch64_frame_pointer_setup(instruction);
        pc = pc.checked_add(4).ok_or_else(|| {
            CodegenError(format!(
                "AArch64 function `{}` address overflows",
                function.symbol
            ))
        })?;
    }
    if !frame_record || !frame_pointer {
        return Err(CodegenError(format!(
            "managed AArch64 function `{}` does not establish an x29/x30 frame chain before its first statepoint",
            function.symbol
        )));
    }
    Ok(())
}

pub(super) fn validate_aarch64_return_pc(
    text: &TextSection,
    function: &FunctionRelocation,
    instruction_offset: u32,
    safepoint: u64,
) -> Result<u64, CodegenError> {
    if instruction_offset % 4 != 0 {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} has unaligned AArch64 instruction offset {instruction_offset}"
        )));
    }
    let return_pc = function
        .address
        .checked_add(u64::from(instruction_offset))
        .ok_or_else(|| CodegenError(format!("SafepointId {safepoint} return PC overflows")))?;
    let call_pc = return_pc.checked_sub(4).ok_or_else(|| {
        CodegenError(format!(
            "SafepointId {safepoint} instruction offset does not name an AArch64 return PC"
        ))
    })?;
    let instruction = text_instruction(text, call_pc, "statepoint callsite")?;
    if !is_aarch64_call(instruction) {
        return Err(CodegenError(format!(
            "SafepointId {safepoint} return PC {return_pc:#x} does not immediately follow an AArch64 bl/blr instruction"
        )));
    }
    Ok(return_pc)
}
