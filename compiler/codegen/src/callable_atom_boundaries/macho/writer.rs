//! Deterministic Mach-O symbol-table rebuild for callable atom boundaries.

use std::collections::BTreeSet;

use object::macho;

use super::{BoundaryDefinitionV1, MachOLayout, NLIST_64_SIZE, malformed, read_u32, write_u32};
use crate::CodegenError;

impl MachOLayout {
    pub(crate) fn add_external_definitions(
        &self,
        bytes: &mut Vec<u8>,
        mut additions: Vec<BoundaryDefinitionV1>,
    ) -> Result<(), CodegenError> {
        validate_additions(self, &mut additions)?;
        self.shift_undefined_relocation_indices(bytes, additions.len())?;

        let symbol_start = usize::try_from(self.symtab.symbol_offset).map_err(|_| malformed())?;
        let old_symbol_bytes = usize::try_from(self.symtab.symbol_count)
            .ok()
            .and_then(|count| count.checked_mul(NLIST_64_SIZE))
            .ok_or_else(malformed)?;
        let undefined_start = usize::try_from(self.dysymtab.undefined_index)
            .ok()
            .and_then(|index| index.checked_mul(NLIST_64_SIZE))
            .ok_or_else(malformed)?;
        let old_symbol_end = symbol_start
            .checked_add(old_symbol_bytes)
            .ok_or_else(malformed)?;
        let old_symbols = bytes
            .get(symbol_start..old_symbol_end)
            .ok_or_else(malformed)?
            .to_vec();
        let old_strings = bytes
            .get(usize::try_from(self.symtab.string_offset).map_err(|_| malformed())?..)
            .ok_or_else(malformed)?
            .to_vec();

        let mut strings = old_strings;
        let encoded = encode_additions(&additions, &mut strings)?;
        let addition_count = u32::try_from(additions.len())
            .map_err(|_| CodegenError("too many callable boundary symbols".to_string()))?;
        let new_symbol_count = self
            .symtab
            .symbol_count
            .checked_add(addition_count)
            .ok_or_else(|| CodegenError("callable symbol count overflows".to_string()))?;
        let new_external_count = self
            .dysymtab
            .external_definition_count
            .checked_add(addition_count)
            .ok_or_else(|| {
                CodegenError("callable external definition count overflows".to_string())
            })?;
        let new_undefined_index = self
            .dysymtab
            .undefined_index
            .checked_add(addition_count)
            .ok_or_else(|| CodegenError("callable undefined symbol index overflows".to_string()))?;

        bytes.truncate(symbol_start);
        bytes.extend_from_slice(old_symbols.get(..undefined_start).ok_or_else(malformed)?);
        bytes.extend_from_slice(&encoded);
        bytes.extend_from_slice(old_symbols.get(undefined_start..).ok_or_else(malformed)?);
        let new_string_offset = u32::try_from(bytes.len())
            .map_err(|_| CodegenError("callable object exceeds Mach-O u32 offsets".to_string()))?;
        bytes.extend_from_slice(&strings);
        let new_string_size = u32::try_from(strings.len())
            .map_err(|_| CodegenError("callable string table exceeds u32".to_string()))?;

        write_u32(bytes, self.symtab.command_offset + 12, new_symbol_count)?;
        write_u32(bytes, self.symtab.command_offset + 16, new_string_offset)?;
        write_u32(bytes, self.symtab.command_offset + 20, new_string_size)?;
        write_u32(bytes, self.dysymtab.command_offset + 20, new_external_count)?;
        write_u32(
            bytes,
            self.dysymtab.command_offset + 24,
            new_undefined_index,
        )?;
        Ok(())
    }

    fn shift_undefined_relocation_indices(
        &self,
        bytes: &mut [u8],
        addition_count: usize,
    ) -> Result<(), CodegenError> {
        let delta = u32::try_from(addition_count)
            .map_err(|_| CodegenError("too many callable boundary symbols".to_string()))?;
        for section in &self.sections {
            for index in 0..section.relocation_count {
                let offset = usize::try_from(section.relocation_offset)
                    .ok()
                    .and_then(|start| {
                        usize::try_from(index)
                            .ok()
                            .and_then(|index| index.checked_mul(8))
                            .and_then(|relative| start.checked_add(relative))
                    })
                    .ok_or_else(malformed)?;
                let address_word = read_u32(bytes, offset)?;
                if address_word & macho::R_SCATTERED != 0 {
                    return Err(CodegenError(
                        "callable object uses a scattered relocation".to_string(),
                    ));
                }
                let info = read_u32(bytes, offset + 4)?;
                if (info >> 27) & 1 == 0 {
                    continue;
                }
                let target = info & 0x00ff_ffff;
                if target >= self.symtab.symbol_count {
                    return Err(CodegenError(
                        "callable relocation symbol index is out of bounds".to_string(),
                    ));
                }
                if target < self.dysymtab.undefined_index {
                    continue;
                }
                let shifted = target
                    .checked_add(delta)
                    .filter(|value| *value <= 0x00ff_ffff)
                    .ok_or_else(|| {
                        CodegenError("callable relocation symbol index overflows".to_string())
                    })?;
                write_u32(bytes, offset + 4, (info & 0xff00_0000) | shifted)?;
            }
        }
        Ok(())
    }
}

fn validate_additions(
    layout: &MachOLayout,
    additions: &mut [BoundaryDefinitionV1],
) -> Result<(), CodegenError> {
    if additions.is_empty() {
        return Err(CodegenError(
            "callable object has no backend boundary symbols to materialize".to_string(),
        ));
    }
    additions.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    if additions
        .windows(2)
        .any(|pair| pair[0].name == pair[1].name)
    {
        return Err(CodegenError(
            "callable boundary plan repeats a Mach-O symbol".to_string(),
        ));
    }
    let existing = layout
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_slice())
        .collect::<BTreeSet<_>>();
    if let Some(collision) = additions
        .iter()
        .find(|addition| existing.contains(addition.name.as_slice()))
    {
        return Err(CodegenError(format!(
            "callable boundary symbol `{}` already exists",
            String::from_utf8_lossy(&collision.name)
        )));
    }
    if additions.iter().any(|addition| {
        addition.name.is_empty()
            || addition.name.contains(&0)
            || addition.section_ordinal == 0
            || usize::from(addition.section_ordinal) > layout.sections.len()
    }) {
        return Err(CodegenError(
            "callable boundary plan contains an invalid Mach-O definition".to_string(),
        ));
    }
    Ok(())
}

fn encode_additions(
    additions: &[BoundaryDefinitionV1],
    strings: &mut Vec<u8>,
) -> Result<Vec<u8>, CodegenError> {
    let capacity = additions
        .len()
        .checked_mul(NLIST_64_SIZE)
        .ok_or_else(|| CodegenError("callable symbol table size overflows".to_string()))?;
    let mut encoded = Vec::with_capacity(capacity);
    for addition in additions {
        let string_index = u32::try_from(strings.len())
            .map_err(|_| CodegenError("callable boundary string table exceeds u32".to_string()))?;
        strings.extend_from_slice(&addition.name);
        strings.push(0);
        encoded.extend_from_slice(&string_index.to_le_bytes());
        encoded.push(macho::N_SECT | macho::N_EXT);
        encoded.push(addition.section_ordinal);
        encoded.extend_from_slice(&0_u16.to_le_bytes());
        encoded.extend_from_slice(&addition.value.to_le_bytes());
    }
    Ok(encoded)
}
