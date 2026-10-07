//! Physical ranges, symbol partitions and section tables in a Mach-O object.

use super::*;

pub(super) fn validate_symbol_table(
    record: &macho::SymtabCommand<Endianness>,
    endian: Endianness,
    bytes: &[u8],
    byte_length: u64,
    occupied_ranges: &mut Vec<CheckedFileRange>,
) -> Result<(), ObjectEnvelopeValidationError> {
    let symbol_count = record.nsyms.get(endian);
    let symbol_bytes = u64::from(symbol_count)
        .checked_mul(mem::size_of::<macho::Nlist64<Endianness>>() as u64)
        .ok_or(ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    let symbol_range = CheckedFileRange::new(u64::from(record.symoff.get(endian)), symbol_bytes)
        .map_err(|_| ObjectEnvelopeValidationError::SymbolTableOutOfBounds)?;
    if symbol_range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::SymbolTableOutOfBounds);
    }
    let string_size = u64::from(record.strsize.get(endian));
    let string_range = CheckedFileRange::new(u64::from(record.stroff.get(endian)), string_size)
        .map_err(|_| ObjectEnvelopeValidationError::StringTableOutOfBounds)?;
    if string_range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::StringTableOutOfBounds);
    }
    if string_range.is_empty() || bytes.get(string_range.start as usize).copied().unwrap_or(1) != 0
    {
        return Err(ObjectEnvelopeValidationError::InvalidStringTable);
    }
    occupied_ranges.extend(
        [symbol_range, string_range]
            .into_iter()
            .filter(|range| !range.is_empty()),
    );
    Ok(())
}

pub(super) fn validate_section(
    section: &macho::Section64<Endianness>,
    endian: Endianness,
    segment_start: u64,
    segment_end: u64,
    byte_length: u64,
    relocation_count: &mut u64,
    occupied_ranges: &mut Vec<CheckedFileRange>,
) -> Result<ObservedMachOSectionV1, ObjectEnvelopeValidationError> {
    if !has_canonical_fixed_name_padding(&section.sectname)
        || !has_canonical_fixed_name_padding(&section.segname)
    {
        return Err(ObjectEnvelopeValidationError::NonCanonicalSectionName);
    }
    if let Some((offset, size)) = section.file_range(endian) {
        let range = CheckedFileRange::new(offset, size)
            .map_err(|_| ObjectEnvelopeValidationError::SectionOutOfBounds)?;
        if range.end > byte_length || range.start < segment_start || range.end > segment_end {
            return Err(ObjectEnvelopeValidationError::SectionOutOfBounds);
        }
        let alignment_power = section.align.get(endian);
        if alignment_power >= u64::BITS
            || (!range.is_empty() && range.start % (1_u64 << alignment_power) != 0)
        {
            return Err(ObjectEnvelopeValidationError::InvalidSectionAlignment {
                power: alignment_power,
            });
        }
        if !range.is_empty() {
            occupied_ranges.push(range);
        }
    } else if section.nreloc.get(endian) != 0 {
        return Err(ObjectEnvelopeValidationError::ZeroFillSectionHasRelocations);
    }

    let count = section.nreloc.get(endian);
    *relocation_count = relocation_count
        .checked_add(u64::from(count))
        .ok_or(ObjectEnvelopeValidationError::RelocationTableOutOfBounds)?;
    let relocation_bytes = u64::from(count)
        .checked_mul(mem::size_of::<macho::Relocation<Endianness>>() as u64)
        .ok_or(ObjectEnvelopeValidationError::RelocationTableOutOfBounds)?;
    let range = CheckedFileRange::new(u64::from(section.reloff.get(endian)), relocation_bytes)
        .map_err(|_| ObjectEnvelopeValidationError::RelocationTableOutOfBounds)?;
    if range.end > byte_length {
        return Err(ObjectEnvelopeValidationError::RelocationTableOutOfBounds);
    }
    if !range.is_empty() {
        occupied_ranges.push(range);
    }
    if section.reserved1.get(endian) != 0
        || section.reserved2.get(endian) != 0
        || section.reserved3.get(endian) != 0
    {
        return Err(ObjectEnvelopeValidationError::UnsupportedSectionReservedFields);
    }
    Ok(ObservedMachOSectionV1 {
        segment_name: section.segname,
        section_name: section.sectname,
        virtual_address: section.addr.get(endian),
        file_offset: section.file_range(endian).map(|(offset, _)| offset),
        relocation_file_offset: u64::from(section.reloff.get(endian)),
        flags: section.flags.get(endian),
        byte_size: section.size.get(endian),
        alignment_power: section.align.get(endian),
        relocation_count: count,
    })
}

fn has_canonical_fixed_name_padding(name: &[u8; 16]) -> bool {
    name.iter()
        .position(|byte| *byte == 0)
        .is_none_or(|first_zero| name[first_zero..].iter().all(|byte| *byte == 0))
}

pub(super) fn validate_dynamic_symbol_table(
    record: &macho::DysymtabCommand<Endianness>,
    endian: Endianness,
    symbol_count: u32,
) -> Result<(), ObjectEnvelopeValidationError> {
    let local_end = record
        .ilocalsym
        .get(endian)
        .checked_add(record.nlocalsym.get(endian));
    let external_end = record
        .iextdefsym
        .get(endian)
        .checked_add(record.nextdefsym.get(endian));
    let undefined_end = record
        .iundefsym
        .get(endian)
        .checked_add(record.nundefsym.get(endian));
    if record.ilocalsym.get(endian) != 0
        || local_end != Some(record.iextdefsym.get(endian))
        || external_end != Some(record.iundefsym.get(endian))
        || undefined_end != Some(symbol_count)
    {
        return Err(ObjectEnvelopeValidationError::InvalidDynamicSymbolPartition);
    }
    if [
        record.tocoff.get(endian),
        record.ntoc.get(endian),
        record.modtaboff.get(endian),
        record.nmodtab.get(endian),
        record.extrefsymoff.get(endian),
        record.nextrefsyms.get(endian),
        record.indirectsymoff.get(endian),
        record.nindirectsyms.get(endian),
        record.extreloff.get(endian),
        record.nextrel.get(endian),
        record.locreloff.get(endian),
        record.nlocrel.get(endian),
    ]
    .iter()
    .any(|value| *value != 0)
    {
        return Err(ObjectEnvelopeValidationError::UnexpectedDynamicLinkTable);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CheckedFileRange {
    pub(super) start: u64,
    pub(super) end: u64,
}

impl CheckedFileRange {
    pub(super) fn new(start: u64, size: u64) -> Result<Self, ObjectEnvelopeValidationError> {
        let end = start
            .checked_add(size)
            .ok_or(ObjectEnvelopeValidationError::FileRangeOverflow)?;
        Ok(Self { start, end })
    }

    pub(super) const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

pub(super) fn validate_disjoint_ranges(
    ranges: &mut [CheckedFileRange],
) -> Result<(), ObjectEnvelopeValidationError> {
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err(ObjectEnvelopeValidationError::OverlappingFileRanges);
    }
    Ok(())
}
