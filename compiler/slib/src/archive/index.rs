//! Directory positions shared by memory and file inputs.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemberPosition {
    pub(crate) ordinal: u32,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    pub(crate) compressed: bool,
}

impl MemberPosition {
    pub(crate) fn ordinal(self) -> ArchiveMemberOrdinal {
        ArchiveMemberOrdinal::Directory(self.ordinal)
    }

    pub(crate) fn range(self) -> Result<Range<usize>, ArchiveReadError> {
        let start = usize::try_from(self.offset).map_err(|_| ArchiveReadError::LengthOverflow)?;
        let length = usize::try_from(self.length).map_err(|_| ArchiveReadError::LengthOverflow)?;
        let end = start
            .checked_add(length)
            .ok_or(ArchiveReadError::LengthOverflow)?;
        Ok(start..end)
    }
}

pub(crate) fn member_positions(
    records: &[SlibMemberRecord],
    mut cursor: u64,
) -> Result<Vec<(SlibMemberId, MemberPosition)>, ArchiveReadError> {
    let mut positions = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if index > 0 && records[index - 1].id() >= record.id() {
            return Err(ArchiveReadError::NonIncreasingDirectory {
                first_index: index - 1,
                second_index: index,
            });
        }
        positions.push((
            record.id(),
            MemberPosition {
                ordinal: u32::try_from(index).map_err(|_| ArchiveReadError::LengthOverflow)?,
                offset: cursor,
                length: record.byte_length(),
                compressed: record.role().is_compressed(),
            },
        ));
        cursor = cursor
            .checked_add(record.byte_length())
            .ok_or(ArchiveReadError::LengthOverflow)?;
    }
    Ok(positions)
}
