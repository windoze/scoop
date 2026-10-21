//! Directory positions shared by memory and file inputs.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemberPosition {
    pub(crate) ordinal: u32,
    pub(crate) header: u64,
    pub(crate) length: u64,
    pub(crate) compressed: bool,
}

impl MemberPosition {
    pub(crate) fn name(self) -> Result<[u8; 9], ArchiveReadError> {
        member_name(self.ordinal as usize).ok_or(ArchiveReadError::LengthOverflow)
    }

    pub(crate) fn ordinal(self) -> ArchiveMemberOrdinal {
        ArchiveMemberOrdinal::Directory(self.ordinal)
    }

    pub(crate) fn check_length(self, actual: usize) -> Result<(), ArchiveReadError> {
        if actual as u64 != self.length {
            return Err(ArchiveReadError::MemberLengthMismatch {
                member: self.ordinal(),
                declared: self.length,
                actual: actual as u64,
            });
        }
        Ok(())
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
                header: cursor,
                length: record.byte_length(),
                compressed: record.role().is_compressed(),
            },
        ));
        cursor = cursor
            .checked_add(
                encoded_member_length(record.byte_length())
                    .ok_or(ArchiveReadError::LengthOverflow)?,
            )
            .ok_or(ArchiveReadError::LengthOverflow)?;
    }
    Ok(positions)
}
