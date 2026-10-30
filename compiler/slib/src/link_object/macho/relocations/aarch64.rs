//! Darwin AArch64 relocation instruction and pair rules.

use super::*;

pub(super) fn decode(
    raw: &[RawRelocationV1],
    index: usize,
    section_count: u32,
    symbol_count: u32,
) -> Result<(usize, MachORelocationShapeV1, usize), MachORelocationInventoryValidationError> {
    let first = raw[index];
    Ok(match first.kind {
        macho::ARM64_RELOC_ADDEND => {
            let second = paired_relocation(raw, index, first)?;
            validate_exact_fields(first, false, 2, false)?;
            let target = validate_target(second, section_count, symbol_count)?;
            let explicit_addend = sign_extend_24(first.target);
            let shape = match second.kind {
                macho::ARM64_RELOC_PAGE21 => {
                    validate_exact_fields(second, true, 2, true)?;
                    MachORelocationShapeV1::Page21 {
                        target,
                        explicit_addend: Some(explicit_addend),
                    }
                }
                macho::ARM64_RELOC_PAGEOFF12 => {
                    validate_exact_fields(second, false, 2, true)?;
                    MachORelocationShapeV1::PageOffset12 {
                        target,
                        explicit_addend: Some(explicit_addend),
                    }
                }
                actual => {
                    return Err(
                        MachORelocationInventoryValidationError::InvalidRelocationPair {
                            first: first.kind,
                            second: actual,
                        },
                    );
                }
            };
            (4, shape, 2)
        }
        macho::ARM64_RELOC_SUBTRACTOR => {
            let second = paired_relocation(raw, index, first)?;
            validate_exact_fields(first, false, 3, true)?;
            if second.kind != macho::ARM64_RELOC_UNSIGNED {
                return Err(
                    MachORelocationInventoryValidationError::InvalidRelocationPair {
                        first: first.kind,
                        second: second.kind,
                    },
                );
            }
            validate_exact_fields(second, false, 3, true)?;
            (
                8,
                MachORelocationShapeV1::Subtractor64 {
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
                MachORelocationShapeV1::Unsigned64 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_BRANCH26 => {
            validate_exact_fields(first, true, 2, true)?;
            (
                4,
                MachORelocationShapeV1::Branch26 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_PAGE21 => {
            validate_exact_fields(first, true, 2, true)?;
            (
                4,
                MachORelocationShapeV1::Page21 {
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
                MachORelocationShapeV1::PageOffset12 {
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
                MachORelocationShapeV1::GotLoadPage21 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_GOT_LOAD_PAGEOFF12 => {
            validate_exact_fields(first, false, 2, true)?;
            (
                4,
                MachORelocationShapeV1::GotLoadPageOffset12 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_POINTER_TO_GOT => {
            validate_exact_fields(first, true, 2, true)?;
            (
                4,
                MachORelocationShapeV1::PointerToGot32 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_TLVP_LOAD_PAGE21 => {
            validate_exact_fields(first, true, 2, true)?;
            (
                4,
                MachORelocationShapeV1::TlvpLoadPage21 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        macho::ARM64_RELOC_TLVP_LOAD_PAGEOFF12 => {
            validate_exact_fields(first, false, 2, true)?;
            (
                4,
                MachORelocationShapeV1::TlvpLoadPageOffset12 {
                    target: validate_target(first, section_count, symbol_count)?,
                },
                1,
            )
        }
        actual => {
            return Err(
                MachORelocationInventoryValidationError::UnsupportedRelocationKind { actual },
            );
        }
    })
}

const fn sign_extend_24(value: u32) -> i32 {
    ((value << 8) as i32) >> 8
}
