//! Validate section padding without assigning empty sentinels to definition atoms.

use super::*;

pub(super) fn validate_and_assign_zero_padding(
    bytes: &[u8],
    sections: &ValidatedBuiltinObjectSectionInventoryV1,
    definitions: &mut [VerifiedStrongObjectDefinitionV1],
) -> Result<(), StrongObjectDefinitionValidationError> {
    let mut ranges_by_section = BTreeMap::<NonZeroU32, Vec<VerifiedDefinitionAtomRangeV1>>::new();
    for range in definitions
        .iter()
        .flat_map(|definition| definition.atoms.iter().copied())
    {
        ranges_by_section
            .entry(range.section_ordinal)
            .or_default()
            .push(range);
    }
    let mut padding_ends = BTreeMap::new();
    for (index, section) in sections.envelope().sections().iter().enumerate() {
        if sections.roles()[index] == BuiltinObjectSectionRoleV1::ObjectMetadata {
            continue;
        }
        let ordinal_index = index + 1;
        let ordinal = u32::try_from(ordinal_index)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(
                StrongObjectDefinitionValidationError::UnsupportedSectionOrdinal {
                    index: u32::try_from(ordinal_index).unwrap_or(u32::MAX),
                },
            )?;
        let section_end = section
            .virtual_address()
            .checked_add(section.byte_size())
            .ok_or(
                StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal },
            )?;
        let Some(ranges) = ranges_by_section.get_mut(&ordinal) else {
            if section.byte_size() != 0 {
                return Err(StrongObjectDefinitionValidationError::UnownedSection {
                    section: ordinal,
                    role: sections.roles()[index],
                    segment_name: section.segment_name().to_vec(),
                    section_name: section.section_name().to_vec(),
                    symbols: sections
                        .envelope()
                        .symbols()
                        .iter()
                        .filter(|symbol| symbol.section_ordinal() == Some(ordinal))
                        .map(|symbol| (symbol.name().to_vec(), symbol.value()))
                        .collect(),
                });
            }
            continue;
        };
        ranges.sort_unstable_by_key(|range| (range.start, range.end, range.atom));
        if ranges[0].start != section.virtual_address()
            && (sections.roles()[index] != BuiltinObjectSectionRoleV1::ReadOnlyData
                || padding_bytes(
                    bytes,
                    section,
                    ordinal,
                    section.virtual_address(),
                    ranges[0].start,
                )?
                .iter()
                .any(|byte| *byte != 0))
        {
            return Err(
                StrongObjectDefinitionValidationError::UnownedSectionPrefix {
                    section: ordinal,
                    section_start: section.virtual_address(),
                    first_atom_start: ranges[0].start,
                },
            );
        }
        for range_index in 0..ranges.len() {
            let range = ranges[range_index];
            let padding_end = ranges
                .get(range_index + 1)
                .map_or(section_end, |next| next.start);
            validate_zero_padding(bytes, section, range, padding_end)?;
            padding_ends.insert(range.atom, padding_end);
        }
    }
    if padding_ends.len()
        != definitions
            .iter()
            .map(|definition| definition.atoms.len())
            .sum::<usize>()
    {
        return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
    }
    for atom in definitions
        .iter_mut()
        .flat_map(|definition| definition.atoms.iter_mut())
    {
        atom.padding_end = padding_ends[&atom.atom];
    }
    Ok(())
}

fn validate_zero_padding(
    bytes: &[u8],
    section: &super::super::ObservedObjectSectionV1,
    range: VerifiedDefinitionAtomRangeV1,
    padding_end: u64,
) -> Result<(), StrongObjectDefinitionValidationError> {
    let padding = padding_bytes(
        bytes,
        section,
        range.section_ordinal,
        range.end,
        padding_end,
    )?;
    if let Some(offset) = padding.iter().position(|byte| *byte != 0) {
        let address = range.end.checked_add(offset as u64).ok_or(
            StrongObjectDefinitionValidationError::InvalidSectionByteRange {
                section: range.section_ordinal,
            },
        )?;
        return Err(StrongObjectDefinitionValidationError::NonzeroAtomPadding {
            atom: range.atom,
            address,
        });
    }
    Ok(())
}

fn padding_bytes<'a>(
    bytes: &'a [u8],
    section: &super::super::ObservedObjectSectionV1,
    ordinal: NonZeroU32,
    start: u64,
    end: u64,
) -> Result<&'a [u8], StrongObjectDefinitionValidationError> {
    let Some(file_offset) = section.file_offset() else {
        return Ok(&[]);
    };
    let padding_start_in_section = start.checked_sub(section.virtual_address()).ok_or(
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal },
    )?;
    let padding_end_in_section = end.checked_sub(section.virtual_address()).ok_or(
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal },
    )?;
    let start = file_offset.checked_add(padding_start_in_section).ok_or(
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal },
    )?;
    let end = file_offset.checked_add(padding_end_in_section).ok_or(
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal },
    )?;
    let start = usize::try_from(start).map_err(|_| {
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal }
    })?;
    let end = usize::try_from(end).map_err(|_| {
        StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal }
    })?;
    bytes
        .get(start..end)
        .ok_or(StrongObjectDefinitionValidationError::InvalidSectionByteRange { section: ordinal })
}
