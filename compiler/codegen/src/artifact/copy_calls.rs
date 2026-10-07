//! Identify the standard, non-unwinding copy libcall introduced for byval.

use std::collections::BTreeSet;

use object::{Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget, elf, macho};

use crate::CodegenError;

pub(super) fn non_unwinding_calls(
    file: &object::File<'_>,
    section: &object::Section<'_, '_>,
) -> Result<BTreeSet<u64>, CodegenError> {
    let bytes = section
        .data()
        .map_err(|error| CodegenError(format!("copy call section: {error}")))?;
    let mut calls = BTreeSet::new();
    for (offset, relocation) in section.relocations() {
        let RelocationTarget::Symbol(index) = relocation.target() else {
            continue;
        };
        let (instruction, name) = match relocation.flags() {
            RelocationFlags::MachO {
                r_type: macho::ARM64_RELOC_BRANCH26,
                r_pcrel: true,
                r_length: 2,
            } if relocation.addend() == 0 => {
                let Some(encoded) = bytes
                    .get(offset as usize..)
                    .and_then(|bytes| bytes.get(..4))
                else {
                    continue;
                };
                if encoded != 0x9400_0000u32.to_le_bytes() {
                    continue;
                }
                (offset, "_memcpy")
            }
            RelocationFlags::Elf {
                r_type: elf::R_X86_64_PLT32 | elf::R_X86_64_PC32,
            } if relocation.addend() == -4 => {
                let Some(instruction) = offset.checked_sub(1) else {
                    continue;
                };
                if bytes.get(instruction as usize) != Some(&0xe8) {
                    continue;
                }
                (instruction, "memcpy")
            }
            _ => continue,
        };
        let symbol = file
            .symbol_by_index(index)
            .map_err(|error| CodegenError(format!("copy call symbol: {error}")))?;
        if symbol.is_undefined() && symbol.name().ok() == Some(name) {
            let address = section
                .address()
                .checked_add(instruction)
                .ok_or_else(|| CodegenError("copy call address overflows".into()))?;
            calls.insert(address);
        }
    }
    Ok(calls)
}
