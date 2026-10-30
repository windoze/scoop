//! Mach-O x86-64 displacement and subtractor-pair rules.

use super::*;

pub(super) fn decode(
    raw: &[RawRelocationV1],
    index: usize,
    section_count: u32,
    symbol_count: u32,
) -> Result<(usize, MachORelocationShapeV1, usize), MachORelocationInventoryValidationError> {
    use MachORelocationShapeV1 as Shape;
    use MachOX86_64PcRelativeKindV1 as Kind;
    let first = raw[index];
    if matches!(
        first.kind,
        macho::X86_64_RELOC_UNSIGNED | macho::X86_64_RELOC_SUBTRACTOR
    ) {
        if !matches!(first.length, 2 | 3) {
            return Err(
                MachORelocationInventoryValidationError::InvalidRelocationFields {
                    kind: first.kind,
                    pcrel: first.pcrel,
                    length: first.length,
                    external: first.external,
                },
            );
        }
        let width = 1usize << first.length;
        if first.kind == macho::X86_64_RELOC_UNSIGNED {
            validate_exact_fields(first, false, first.length, first.external)?;
            let target = validate_target(first, section_count, symbol_count)?;
            return Ok((
                width,
                if width == 8 {
                    Shape::Unsigned64 { target }
                } else {
                    Shape::Unsigned32 { target }
                },
                1,
            ));
        }
        validate_exact_fields(first, false, first.length, first.external)?;
        let second = paired_relocation(raw, index, first)?;
        if second.kind != macho::X86_64_RELOC_UNSIGNED {
            return Err(
                MachORelocationInventoryValidationError::InvalidRelocationPair {
                    first: first.kind,
                    second: second.kind,
                },
            );
        }
        validate_exact_fields(second, false, first.length, second.external)?;
        let minuend = validate_target(second, section_count, symbol_count)?;
        let subtrahend = validate_target(first, section_count, symbol_count)?;
        return Ok((
            width,
            if width == 8 {
                Shape::Subtractor64 {
                    minuend,
                    subtrahend,
                }
            } else {
                Shape::Subtractor32 {
                    minuend,
                    subtrahend,
                }
            },
            2,
        ));
    }
    let kind = match first.kind {
        macho::X86_64_RELOC_SIGNED => Kind::Signed,
        macho::X86_64_RELOC_BRANCH => Kind::Branch,
        macho::X86_64_RELOC_GOT_LOAD => Kind::GotLoad,
        macho::X86_64_RELOC_GOT => Kind::Got,
        macho::X86_64_RELOC_SIGNED_1 => Kind::Signed1,
        macho::X86_64_RELOC_SIGNED_2 => Kind::Signed2,
        macho::X86_64_RELOC_SIGNED_4 => Kind::Signed4,
        macho::X86_64_RELOC_TLV => Kind::Tlv,
        actual => {
            return Err(
                MachORelocationInventoryValidationError::UnsupportedRelocationKind { actual },
            );
        }
    };
    let external = matches!(kind, Kind::Got | Kind::GotLoad | Kind::Tlv) || first.external;
    validate_exact_fields(first, true, 2, external)?;
    Ok((
        4,
        Shape::X86_64PcRelative32 {
            kind,
            target: validate_target(first, section_count, symbol_count)?,
        },
        1,
    ))
}
