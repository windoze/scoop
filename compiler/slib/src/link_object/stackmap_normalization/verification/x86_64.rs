//! Bounded amd64 instruction decoding without an LLVM dependency.

use std::collections::BTreeSet;

use yaxpeax_arch::LengthedInstruction;
use yaxpeax_x86::long_mode::{InstDecoder, Opcode, Operand, RegSpec};

use crate::link_object::{
    BuiltinObjectSectionRoleV1, ParsedLlvmStackmapFunctionV3,
    ValidatedBuiltinObjectSectionInventoryV1, VerifiedStrongDefinitionSymbolV1,
    VerifiedStrongObjectDefinitionV1,
};

use X86_64StackmapMachineCodeError as Error;

pub(super) fn validate_stackmap_function_machine_code(
    bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    definition: &VerifiedStrongObjectDefinitionV1,
    symbol: &VerifiedStrongDefinitionSymbolV1,
    function: &ParsedLlvmStackmapFunctionV3,
) -> Result<Vec<u64>, Error> {
    let primary = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == definition.primary_atom())
        .ok_or(Error::InvalidFunctionExtent)?;
    if primary.section_ordinal() != symbol.section_ordinal()
        || primary.start() != symbol.value()
        || primary.start() >= primary.end()
    {
        return Err(Error::InvalidFunctionExtent);
    }
    let index = primary.section_ordinal().get() as usize - 1;
    if sections.roles().get(index) != Some(&BuiltinObjectSectionRoleV1::Text) {
        return Err(Error::InvalidFunctionExtent);
    }
    let section = &sections.envelope().sections()[index];
    let offset = primary.start().checked_sub(section.virtual_address());
    let start = offset
        .and_then(|offset| section.file_offset()?.checked_add(offset))
        .and_then(|start| usize::try_from(start).ok())
        .ok_or(Error::InvalidFunctionExtent)?;
    let size = usize::try_from(primary.end() - primary.start())
        .map_err(|_| Error::InvalidFunctionExtent)?;
    let body = start
        .checked_add(size)
        .and_then(|end| bytes.get(start..end))
        .ok_or(Error::InvalidFunctionExtent)?;
    let offsets = function
        .records()
        .iter()
        .map(|record| record.instruction_offset())
        .collect::<Vec<_>>();
    validate_calls(body, &offsets)?;
    offsets
        .into_iter()
        .map(|offset| {
            primary
                .start()
                .checked_add(u64::from(offset))
                .ok_or(Error::InvalidFunctionExtent)
        })
        .collect()
}

fn validate_calls(bytes: &[u8], return_offsets: &[u32]) -> Result<(), Error> {
    let end = return_offsets
        .iter()
        .copied()
        .max()
        .ok_or(Error::EmptyRecordSet)? as usize;
    if end > bytes.len() || return_offsets.contains(&0) {
        return Err(Error::ReturnPcOutsideFunction);
    }
    let decoder = InstDecoder::default();
    let mut cursor = 0;
    let mut calls = BTreeSet::new();
    let mut saved = false;
    let mut frame = false;
    while cursor < end {
        let instruction =
            decoder
                .decode_slice(&bytes[cursor..end])
                .map_err(|_| Error::InvalidInstruction {
                    offset: cursor as u64,
                })?;
        let length = instruction.len().to_const() as usize;
        if length == 0 || length > end - cursor {
            return Err(Error::InvalidInstruction {
                offset: cursor as u64,
            });
        }
        if calls.is_empty() {
            saved |= instruction.opcode() == Opcode::PUSH
                && instruction.operand(0)
                    == (Operand::Register {
                        reg: RegSpec::rbp(),
                    });
            frame |= saved
                && instruction.opcode() == Opcode::MOV
                && instruction.operand(0)
                    == (Operand::Register {
                        reg: RegSpec::rbp(),
                    })
                && instruction.operand(1)
                    == (Operand::Register {
                        reg: RegSpec::rsp(),
                    });
        }
        cursor += length;
        if instruction.opcode() == Opcode::CALL {
            if !frame {
                return Err(Error::MissingManagedFrameChain);
            }
            calls.insert(cursor as u32);
        }
    }
    for offset in return_offsets {
        if !calls.contains(offset) {
            return Err(Error::ReturnPcDoesNotFollowCall { offset: *offset });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86_64StackmapMachineCodeError {
    InvalidFunctionExtent,
    InvalidInstruction { offset: u64 },
    MissingManagedFrameChain,
    ReturnPcDoesNotFollowCall { offset: u32 },
    ReturnPcOutsideFunction,
    EmptyRecordSet,
}

impl std::fmt::Display for X86_64StackmapMachineCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid managed amd64 machine code: {self:?}")
    }
}
impl std::error::Error for X86_64StackmapMachineCodeError {}

#[cfg(test)]
mod tests;
