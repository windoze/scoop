use crate::link_object::{
    BuiltinObjectSectionRoleV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};

#[derive(Debug)]
pub(super) enum AtomFileRangeError {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

pub(super) fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), AtomFileRangeError> {
    let index = (atom.section_ordinal().get() as usize) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .ok_or(AtomFileRangeError::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(AtomFileRangeError::MissingSection)?;
    let section_file_offset = section
        .file_offset()
        .ok_or(AtomFileRangeError::NotFileBacked)?;
    let start_in_section = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(AtomFileRangeError::InvalidRange)?;
    let end_in_section = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(AtomFileRangeError::InvalidRange)?;
    if end_in_section > section.byte_size() || start_in_section > end_in_section {
        return Err(AtomFileRangeError::InvalidRange);
    }
    let start = section_file_offset
        .checked_add(start_in_section)
        .ok_or(AtomFileRangeError::InvalidRange)?;
    let end = section_file_offset
        .checked_add(end_in_section)
        .ok_or(AtomFileRangeError::InvalidRange)?;
    Ok((role, start, end))
}
