//! Shared Mach-O relocation tables with explicit machine decoding.

use std::num::NonZeroU32;

use crate::link_object::ObjectRelocationTargetV1;
use object::macho;

use super::{MachOArchitecture, ObservedMachOSectionV1};

mod aarch64;
mod types;
pub use types::*;

pub(super) fn validate_macho_relocation_inventory_v1(
    architecture: MachOArchitecture,
    bytes: &[u8],
    sections: &[ObservedMachOSectionV1],
    symbol_count: u32,
) -> Result<Vec<ObservedMachORelocationV1>, MachORelocationInventoryValidationError> {
    let section_count = u32::try_from(sections.len())
        .map_err(|_| MachORelocationInventoryValidationError::TooManySections)?;
    let mut inventory = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let ordinal = u32::try_from(index + 1)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(MachORelocationInventoryValidationError::TooManySections)?;
        let table_start = usize::try_from(section.relocation_file_offset())
            .map_err(|_| MachORelocationInventoryValidationError::RelocationTableOutOfBounds)?;
        let table_size = usize::try_from(section.relocation_count())
            .ok()
            .and_then(|count| count.checked_mul(8))
            .ok_or(MachORelocationInventoryValidationError::RelocationTableOutOfBounds)?;
        let table = bytes
            .get(
                table_start
                    ..table_start.checked_add(table_size).ok_or(
                        MachORelocationInventoryValidationError::RelocationTableOutOfBounds,
                    )?,
            )
            .ok_or(MachORelocationInventoryValidationError::RelocationTableOutOfBounds)?;
        let data = section
            .file_offset()
            .and_then(|start| {
                let end = start.checked_add(section.byte_size())?;
                bytes.get(usize::try_from(start).ok()?..usize::try_from(end).ok()?)
            })
            .unwrap_or(&[]);
        inventory.extend(decode_macho_section_relocations_v1(
            architecture,
            ordinal,
            data,
            table,
            section_count,
            symbol_count,
        )?);
    }
    Ok(inventory)
}

/// Decode one raw relocation table, retaining paired addends and checking every entry.
pub fn decode_macho_section_relocations_v1(
    architecture: MachOArchitecture,
    containing_section_ordinal: NonZeroU32,
    section_data: &[u8],
    table_bytes: &[u8],
    section_count: u32,
    symbol_count: u32,
) -> Result<Vec<ObservedMachORelocationV1>, MachORelocationInventoryValidationError> {
    if table_bytes.len() % 8 != 0 {
        return Err(MachORelocationInventoryValidationError::RelocationTableOutOfBounds);
    }
    let mut inventory = Vec::new();
    let raw = table_bytes
        .chunks_exact(8)
        .map(RawRelocationV1::parse)
        .collect::<Vec<_>>();
    let mut index = 0;
    while index < raw.len() {
        let first = raw[index];
        if first.scattered {
            return Err(MachORelocationInventoryValidationError::ScatteredRelocation);
        }
        let (width, shape, consumed) = match architecture {
            MachOArchitecture::Aarch64 => {
                aarch64::decode(&raw, index, section_count, symbol_count)?
            }
        };
        let encoded_value = read_encoded_value(section_data, first.offset, width)?;
        inventory.push(ObservedMachORelocationV1 {
            containing_section_ordinal,
            offset: first.offset,
            encoded_value,
            shape,
        });
        index += consumed;
    }
    inventory.sort_unstable_by_key(|relocation| relocation.offset);
    if inventory.windows(2).any(|pair| {
        u64::from(pair[0].offset) + u64::from(pair[0].shape.width_bytes())
            > u64::from(pair[1].offset)
    }) {
        return Err(MachORelocationInventoryValidationError::DuplicateRelocationOffset);
    }
    Ok(inventory)
}

fn paired_relocation(
    raw: &[RawRelocationV1],
    index: usize,
    first: RawRelocationV1,
) -> Result<RawRelocationV1, MachORelocationInventoryValidationError> {
    let second = raw.get(index + 1).copied().ok_or(
        MachORelocationInventoryValidationError::MissingRelocationPair { first: first.kind },
    )?;
    if second.scattered || second.offset != first.offset {
        return Err(
            MachORelocationInventoryValidationError::InvalidRelocationPair {
                first: first.kind,
                second: second.kind,
            },
        );
    }
    Ok(second)
}

fn validate_exact_fields(
    relocation: RawRelocationV1,
    pcrel: bool,
    length: u8,
    external: bool,
) -> Result<(), MachORelocationInventoryValidationError> {
    if relocation.pcrel != pcrel || relocation.length != length || relocation.external != external {
        return Err(
            MachORelocationInventoryValidationError::InvalidRelocationFields {
                kind: relocation.kind,
                pcrel: relocation.pcrel,
                length: relocation.length,
                external: relocation.external,
            },
        );
    }
    Ok(())
}

fn validate_target(
    relocation: RawRelocationV1,
    section_count: u32,
    symbol_count: u32,
) -> Result<ObjectRelocationTargetV1, MachORelocationInventoryValidationError> {
    if relocation.external {
        if relocation.target >= symbol_count {
            return Err(
                MachORelocationInventoryValidationError::SymbolTargetOutOfBounds {
                    index: relocation.target,
                },
            );
        }
        Ok(ObjectRelocationTargetV1::SymbolTableIndex(
            relocation.target,
        ))
    } else {
        let ordinal =
            NonZeroU32::new(relocation.target).filter(|value| value.get() <= section_count);
        ordinal.map(ObjectRelocationTargetV1::SectionOrdinal).ok_or(
            MachORelocationInventoryValidationError::SectionTargetOutOfBounds {
                ordinal: relocation.target,
            },
        )
    }
}

fn read_encoded_value(
    bytes: &[u8],
    offset: u32,
    width: usize,
) -> Result<u64, MachORelocationInventoryValidationError> {
    let start = usize::try_from(offset)
        .map_err(|_| MachORelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let end = start
        .checked_add(width)
        .ok_or(MachORelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let site = bytes
        .get(start..end)
        .ok_or(MachORelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let mut value = [0; 8];
    value[..width].copy_from_slice(site);
    Ok(u64::from_le_bytes(value))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawRelocationV1 {
    offset: u32,
    target: u32,
    pcrel: bool,
    length: u8,
    external: bool,
    kind: u8,
    scattered: bool,
}

impl RawRelocationV1 {
    fn parse(bytes: &[u8]) -> Self {
        let word0 = u32::from_le_bytes(bytes[..4].try_into().expect("four-byte relocation word"));
        let word1 = u32::from_le_bytes(bytes[4..].try_into().expect("four-byte relocation word"));
        Self {
            offset: word0,
            target: word1 & 0x00ff_ffff,
            pcrel: (word1 >> 24) & 1 != 0,
            length: ((word1 >> 25) & 3) as u8,
            external: (word1 >> 27) & 1 != 0,
            kind: (word1 >> 28) as u8,
            scattered: word0 & macho::R_SCATTERED != 0,
        }
    }
}

#[cfg(test)]
mod tests;
