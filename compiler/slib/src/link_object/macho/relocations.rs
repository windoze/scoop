//! Closed ARM64 relocation shapes for built-in Mach-O object capabilities.

use std::fmt;
use std::num::NonZeroU32;

use object::macho;

use super::ObservedMachOSectionV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationTargetV1 {
    SymbolTableIndex(u32),
    SectionOrdinal(NonZeroU32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationShapeV1 {
    Unsigned64 {
        target: DarwinArm64RelocationTargetV1,
    },
    Subtractor64 {
        minuend: DarwinArm64RelocationTargetV1,
        subtrahend: DarwinArm64RelocationTargetV1,
    },
    Branch26 {
        target: DarwinArm64RelocationTargetV1,
    },
    Page21 {
        target: DarwinArm64RelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    PageOffset12 {
        target: DarwinArm64RelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    GotLoadPage21 {
        target: DarwinArm64RelocationTargetV1,
    },
    GotLoadPageOffset12 {
        target: DarwinArm64RelocationTargetV1,
    },
    PointerToGot32 {
        target: DarwinArm64RelocationTargetV1,
    },
}

impl DarwinArm64RelocationShapeV1 {
    pub const fn width_bytes(self) -> u8 {
        match self {
            Self::Unsigned64 { .. } | Self::Subtractor64 { .. } => 8,
            Self::Branch26 { .. }
            | Self::Page21 { .. }
            | Self::PageOffset12 { .. }
            | Self::GotLoadPage21 { .. }
            | Self::GotLoadPageOffset12 { .. }
            | Self::PointerToGot32 { .. } => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedMachORelocationV1 {
    containing_section_ordinal: NonZeroU32,
    offset: u32,
    encoded_value: u64,
    shape: DarwinArm64RelocationShapeV1,
}

impl ObservedMachORelocationV1 {
    pub const fn containing_section_ordinal(&self) -> NonZeroU32 {
        self.containing_section_ordinal
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }

    pub const fn shape(&self) -> DarwinArm64RelocationShapeV1 {
        self.shape
    }
}

pub(super) fn validate_darwin_arm64_relocation_inventory_v1(
    bytes: &[u8],
    sections: &[ObservedMachOSectionV1],
    symbol_count: u32,
) -> Result<Vec<ObservedMachORelocationV1>, DarwinArm64RelocationInventoryValidationError> {
    let section_count = u32::try_from(sections.len())
        .map_err(|_| DarwinArm64RelocationInventoryValidationError::TooManySections)?;
    let mut inventory = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let ordinal = u32::try_from(index + 1)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(DarwinArm64RelocationInventoryValidationError::TooManySections)?;
        normalize_section_relocations(
            bytes,
            ordinal,
            *section,
            section_count,
            symbol_count,
            &mut inventory,
        )?;
    }
    inventory.sort_unstable_by_key(|relocation| {
        (relocation.containing_section_ordinal, relocation.offset)
    });
    if inventory.windows(2).any(|pair| {
        pair[0].containing_section_ordinal == pair[1].containing_section_ordinal
            && pair[0].offset == pair[1].offset
    }) {
        return Err(DarwinArm64RelocationInventoryValidationError::DuplicateRelocationOffset);
    }
    Ok(inventory)
}

fn normalize_section_relocations(
    bytes: &[u8],
    containing_section_ordinal: NonZeroU32,
    section: ObservedMachOSectionV1,
    section_count: u32,
    symbol_count: u32,
    inventory: &mut Vec<ObservedMachORelocationV1>,
) -> Result<(), DarwinArm64RelocationInventoryValidationError> {
    let table_start = usize::try_from(section.relocation_file_offset())
        .map_err(|_| DarwinArm64RelocationInventoryValidationError::RelocationTableOutOfBounds)?;
    let count = usize::try_from(section.relocation_count())
        .map_err(|_| DarwinArm64RelocationInventoryValidationError::RelocationTableOutOfBounds)?;
    let table_bytes = count
        .checked_mul(8)
        .and_then(|size| bytes.get(table_start..table_start.checked_add(size)?))
        .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationTableOutOfBounds)?;
    let raw = table_bytes
        .chunks_exact(8)
        .map(RawRelocationV1::parse)
        .collect::<Vec<_>>();
    let mut index = 0;
    while index < raw.len() {
        let first = raw[index];
        if first.scattered {
            return Err(DarwinArm64RelocationInventoryValidationError::ScatteredRelocation);
        }
        let (width, shape, consumed) = match first.kind {
            macho::ARM64_RELOC_ADDEND => {
                let second = paired_relocation(&raw, index, first)?;
                validate_exact_fields(first, false, 2, false)?;
                let target = validate_target(second, section_count, symbol_count)?;
                let explicit_addend = sign_extend_24(first.target);
                let shape = match second.kind {
                    macho::ARM64_RELOC_PAGE21 => {
                        validate_exact_fields(second, true, 2, true)?;
                        DarwinArm64RelocationShapeV1::Page21 {
                            target,
                            explicit_addend: Some(explicit_addend),
                        }
                    }
                    macho::ARM64_RELOC_PAGEOFF12 => {
                        validate_exact_fields(second, false, 2, true)?;
                        DarwinArm64RelocationShapeV1::PageOffset12 {
                            target,
                            explicit_addend: Some(explicit_addend),
                        }
                    }
                    actual => {
                        return Err(
                            DarwinArm64RelocationInventoryValidationError::InvalidRelocationPair {
                                first: first.kind,
                                second: actual,
                            },
                        );
                    }
                };
                (4, shape, 2)
            }
            macho::ARM64_RELOC_SUBTRACTOR => {
                let second = paired_relocation(&raw, index, first)?;
                validate_exact_fields(first, false, 3, true)?;
                if second.kind != macho::ARM64_RELOC_UNSIGNED {
                    return Err(
                        DarwinArm64RelocationInventoryValidationError::InvalidRelocationPair {
                            first: first.kind,
                            second: second.kind,
                        },
                    );
                }
                validate_exact_fields(second, false, 3, true)?;
                (
                    8,
                    DarwinArm64RelocationShapeV1::Subtractor64 {
                        minuend: validate_target(second, section_count, symbol_count)?,
                        subtrahend: validate_target(first, section_count, symbol_count)?,
                    },
                    2,
                )
            }
            macho::ARM64_RELOC_UNSIGNED => {
                validate_exact_fields(first, false, 3, first.external)?;
                (
                    8,
                    DarwinArm64RelocationShapeV1::Unsigned64 {
                        target: validate_target(first, section_count, symbol_count)?,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_BRANCH26 => {
                validate_exact_fields(first, true, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::Branch26 {
                        target: validate_target(first, section_count, symbol_count)?,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_PAGE21 => {
                validate_exact_fields(first, true, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::Page21 {
                        target: validate_target(first, section_count, symbol_count)?,
                        explicit_addend: None,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_PAGEOFF12 => {
                validate_exact_fields(first, false, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::PageOffset12 {
                        target: validate_target(first, section_count, symbol_count)?,
                        explicit_addend: None,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_GOT_LOAD_PAGE21 => {
                validate_exact_fields(first, true, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::GotLoadPage21 {
                        target: validate_target(first, section_count, symbol_count)?,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_GOT_LOAD_PAGEOFF12 => {
                validate_exact_fields(first, false, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::GotLoadPageOffset12 {
                        target: validate_target(first, section_count, symbol_count)?,
                    },
                    1,
                )
            }
            macho::ARM64_RELOC_POINTER_TO_GOT => {
                validate_exact_fields(first, true, 2, true)?;
                (
                    4,
                    DarwinArm64RelocationShapeV1::PointerToGot32 {
                        target: validate_target(first, section_count, symbol_count)?,
                    },
                    1,
                )
            }
            actual => {
                return Err(
                    DarwinArm64RelocationInventoryValidationError::UnsupportedRelocationKind {
                        actual,
                    },
                );
            }
        };
        let encoded_value = read_encoded_value(bytes, section, first.offset, width)?;
        inventory.push(ObservedMachORelocationV1 {
            containing_section_ordinal,
            offset: first.offset,
            encoded_value,
            shape,
        });
        index += consumed;
    }
    Ok(())
}

fn paired_relocation(
    raw: &[RawRelocationV1],
    index: usize,
    first: RawRelocationV1,
) -> Result<RawRelocationV1, DarwinArm64RelocationInventoryValidationError> {
    let second = raw.get(index + 1).copied().ok_or(
        DarwinArm64RelocationInventoryValidationError::MissingRelocationPair { first: first.kind },
    )?;
    if second.scattered || second.offset != first.offset {
        return Err(
            DarwinArm64RelocationInventoryValidationError::InvalidRelocationPair {
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
) -> Result<(), DarwinArm64RelocationInventoryValidationError> {
    if relocation.pcrel != pcrel || relocation.length != length || relocation.external != external {
        return Err(
            DarwinArm64RelocationInventoryValidationError::InvalidRelocationFields {
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
) -> Result<DarwinArm64RelocationTargetV1, DarwinArm64RelocationInventoryValidationError> {
    if relocation.external {
        if relocation.target >= symbol_count {
            return Err(
                DarwinArm64RelocationInventoryValidationError::SymbolTargetOutOfBounds {
                    index: relocation.target,
                },
            );
        }
        Ok(DarwinArm64RelocationTargetV1::SymbolTableIndex(
            relocation.target,
        ))
    } else {
        let ordinal =
            NonZeroU32::new(relocation.target).filter(|value| value.get() <= section_count);
        ordinal
            .map(DarwinArm64RelocationTargetV1::SectionOrdinal)
            .ok_or(
                DarwinArm64RelocationInventoryValidationError::SectionTargetOutOfBounds {
                    ordinal: relocation.target,
                },
            )
    }
}

fn read_encoded_value(
    bytes: &[u8],
    section: ObservedMachOSectionV1,
    offset: u32,
    width: usize,
) -> Result<u64, DarwinArm64RelocationInventoryValidationError> {
    let section_offset = section
        .file_offset()
        .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let relative_end = u64::from(offset)
        .checked_add(width as u64)
        .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    if relative_end > section.byte_size() {
        return Err(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds);
    }
    let start = usize::try_from(
        section_offset
            .checked_add(u64::from(offset))
            .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?,
    )
    .map_err(|_| DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let end = start
        .checked_add(width)
        .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let site = bytes
        .get(start..end)
        .ok_or(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)?;
    let mut value = [0; 8];
    value[..width].copy_from_slice(site);
    Ok(u64::from_le_bytes(value))
}

const fn sign_extend_24(value: u32) -> i32 {
    ((value << 8) as i32) >> 8
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinArm64RelocationInventoryValidationError {
    TooManySections,
    RelocationTableOutOfBounds,
    ScatteredRelocation,
    UnsupportedRelocationKind {
        actual: u8,
    },
    InvalidRelocationFields {
        kind: u8,
        pcrel: bool,
        length: u8,
        external: bool,
    },
    MissingRelocationPair {
        first: u8,
    },
    InvalidRelocationPair {
        first: u8,
        second: u8,
    },
    SymbolTargetOutOfBounds {
        index: u32,
    },
    SectionTargetOutOfBounds {
        ordinal: u32,
    },
    RelocationSiteOutOfBounds,
    DuplicateRelocationOffset,
}

impl fmt::Display for DarwinArm64RelocationInventoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Darwin/AArch64 object relocation inventory: {self:?}"
        )
    }
}

impl std::error::Error for DarwinArm64RelocationInventoryValidationError {}

#[cfg(test)]
mod tests;
