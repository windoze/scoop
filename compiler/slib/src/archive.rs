use std::fmt;
use std::ops::Range;

use scoop_wire::{WireError, WirePath};

use crate::{SlibMember, SlibMemberId, SlibMemberRecord};

pub(crate) const GLOBAL_MAGIC: &[u8; 8] = b"!<arch>\n";
pub(crate) const HEADER_LENGTH: u64 = 60;

mod write;
pub use write::{ArchiveWriteError, CanonicalSlibArchive};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveMemberOrdinal {
    Manifest,
    Directory(u32),
}

impl fmt::Display for ArchiveMemberOrdinal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest => formatter.write_str("manifest"),
            Self::Directory(ordinal) => write!(formatter, "directory member {ordinal}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveReadError {
    BadMagic,
    TruncatedHeader {
        member: ArchiveMemberOrdinal,
    },
    NonCanonicalHeader {
        member: ArchiveMemberOrdinal,
    },
    InvalidSize {
        member: ArchiveMemberOrdinal,
    },

    TruncatedPayload {
        member: ArchiveMemberOrdinal,
    },
    InvalidPadding {
        member: ArchiveMemberOrdinal,
    },

    NonIncreasingDirectory {
        first_index: usize,
        second_index: usize,
    },
    PredictedLengthMismatch {
        predicted: u64,
        actual: u64,
    },
    MemberLengthMismatch {
        member: ArchiveMemberOrdinal,
        declared: u64,
        actual: u64,
    },
    LengthOverflow,
    Allocation(WireError),
}

impl fmt::Display for ArchiveReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("archive global magic is not canonical"),
            Self::TruncatedHeader { member } => {
                write!(formatter, "{member} has a truncated archive header")
            }
            Self::NonCanonicalHeader { member } => {
                write!(formatter, "{member} archive header is not canonical")
            }
            Self::InvalidSize { member } => {
                write!(formatter, "{member} archive size field is invalid")
            }

            Self::TruncatedPayload { member } => {
                write!(formatter, "{member} payload is truncated")
            }
            Self::InvalidPadding { member } => {
                write!(formatter, "{member} archive padding is not canonical")
            }

            Self::NonIncreasingDirectory {
                first_index,
                second_index,
            } => write!(
                formatter,
                "directory member ids {first_index} and {second_index} are not strictly increasing"
            ),
            Self::PredictedLengthMismatch { predicted, actual } => write!(
                formatter,
                "directory predicts archive length {predicted}, found {actual}"
            ),
            Self::MemberLengthMismatch {
                member,
                declared,
                actual,
            } => write!(
                formatter,
                "{member} header length {actual} does not match directory length {declared}"
            ),
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArchiveReadError {}

/// Canonical physical archive whose first member has been proven to be the
/// unique `manifest.cbor` member. No manifest semantics are trusted yet.
#[derive(Debug, Eq, PartialEq)]
pub struct ManifestArchive<'input> {
    input: &'input [u8],
    manifest: Range<usize>,
    next_header: usize,
}

impl<'input> ManifestArchive<'input> {
    pub fn open(input: &'input [u8]) -> Result<Self, ArchiveReadError> {
        if input.get(..GLOBAL_MAGIC.len()) != Some(GLOBAL_MAGIC) {
            return Err(ArchiveReadError::BadMagic);
        }

        let ordinal = ArchiveMemberOrdinal::Manifest;
        let (manifest, next_header) =
            read_member(input, GLOBAL_MAGIC.len(), b"manifest.cbor", ordinal)?;

        Ok(Self {
            input,
            manifest,
            next_header,
        })
    }

    pub fn manifest(&self) -> &'input [u8] {
        &self.input[self.manifest.clone()]
    }

    pub fn validate_directory(
        self,
        records: &[SlibMemberRecord],
    ) -> Result<DecodedArchive<'input>, ArchiveReadError> {
        let path = WirePath::default();
        let record_count =
            u64::try_from(records.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;

        for (index, pair) in records.windows(2).enumerate() {
            if pair[0].id() >= pair[1].id() {
                return Err(ArchiveReadError::NonIncreasingDirectory {
                    first_index: index,
                    second_index: index + 1,
                });
            }
        }

        let mut predicted_length =
            u64::try_from(self.next_header).map_err(|_| ArchiveReadError::LengthOverflow)?;
        for record in records {
            predicted_length = predicted_length
                .checked_add(
                    encoded_member_length(record.byte_length())
                        .ok_or(ArchiveReadError::LengthOverflow)?,
                )
                .ok_or(ArchiveReadError::LengthOverflow)?;
        }
        let actual_length =
            u64::try_from(self.input.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;
        if predicted_length != actual_length {
            return Err(ArchiveReadError::PredictedLengthMismatch {
                predicted: predicted_length,
                actual: actual_length,
            });
        }

        let mut ranges = Vec::new();

        scoop_wire::allocation::try_reserve_count(&mut ranges, record_count, &path)
            .map_err(ArchiveReadError::Allocation)?;
        let mut cursor = self.next_header;
        for (index, record) in records.iter().enumerate() {
            let ordinal = u32::try_from(index).map_err(|_| ArchiveReadError::LengthOverflow)?;
            let member = ArchiveMemberOrdinal::Directory(ordinal);
            let name = member_name(index).ok_or(ArchiveReadError::LengthOverflow)?;
            let (range, next_header) = read_member(self.input, cursor, &name, member)?;
            let actual =
                u64::try_from(range.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;
            if actual != record.byte_length() {
                return Err(ArchiveReadError::MemberLengthMismatch {
                    member,
                    declared: record.byte_length(),
                    actual,
                });
            }

            ranges.push((record.id(), range));
            cursor = next_header;
        }
        debug_assert_eq!(cursor, self.input.len());
        Ok(DecodedArchive {
            input: self.input,
            manifest: self.manifest,
            members: ranges,
        })
    }
}

/// Archive member ranges indexed by the manifest directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedArchive<'input> {
    input: &'input [u8],
    manifest: Range<usize>,
    members: Vec<(SlibMemberId, Range<usize>)>,
}

impl<'input> DecodedArchive<'input> {
    pub fn manifest(&self) -> &'input [u8] {
        &self.input[self.manifest.clone()]
    }

    pub fn member_ids(&self) -> impl ExactSizeIterator<Item = SlibMemberId> + '_ {
        self.members.iter().map(|(id, _)| *id)
    }

    pub(crate) fn member(&self, id: SlibMemberId) -> Option<&'input [u8]> {
        let index = self
            .members
            .binary_search_by_key(&id, |(member, _)| *member)
            .ok()?;
        Some(&self.input[self.members[index].1.clone()])
    }
}

fn read_member(
    input: &[u8],
    header_start: usize,
    expected_name: &[u8],
    member: ArchiveMemberOrdinal,
) -> Result<(Range<usize>, usize), ArchiveReadError> {
    let header_length =
        usize::try_from(HEADER_LENGTH).map_err(|_| ArchiveReadError::LengthOverflow)?;
    let header_end = header_start
        .checked_add(header_length)
        .ok_or(ArchiveReadError::LengthOverflow)?;
    let header = input
        .get(header_start..header_end)
        .ok_or(ArchiveReadError::TruncatedHeader { member })?;

    let payload_length_usize = read_member_header(header, expected_name, member)?;
    let payload_end = header_end
        .checked_add(payload_length_usize)
        .ok_or(ArchiveReadError::LengthOverflow)?;
    if input.get(header_end..payload_end).is_none() {
        return Err(ArchiveReadError::TruncatedPayload { member });
    }
    let next_header = if payload_length_usize & 1 == 1 {
        let padding_end = payload_end
            .checked_add(1)
            .ok_or(ArchiveReadError::LengthOverflow)?;
        if input.get(payload_end..padding_end) != Some(b"\n") {
            return Err(ArchiveReadError::InvalidPadding { member });
        }
        padding_end
    } else {
        payload_end
    };
    Ok((header_end..payload_end, next_header))
}

pub(crate) fn read_member_header(
    header: &[u8],
    expected_name: &[u8],
    member: ArchiveMemberOrdinal,
) -> Result<usize, ArchiveReadError> {
    if header.len() != HEADER_LENGTH as usize {
        return Err(ArchiveReadError::TruncatedHeader { member });
    }
    if !is_canonical_archive_name(&header[0..16], expected_name)
        || !is_canonical_field(&header[16..28], b"0")
        || !is_canonical_field(&header[28..34], b"0")
        || !is_canonical_field(&header[34..40], b"0")
        || !is_canonical_field(&header[40..48], b"100644")
        || &header[58..60] != b"`\n"
    {
        return Err(ArchiveReadError::NonCanonicalHeader { member });
    }
    let payload_length =
        parse_size(&header[48..58]).ok_or(ArchiveReadError::InvalidSize { member })?;
    usize::try_from(payload_length).map_err(|_| ArchiveReadError::LengthOverflow)
}

fn is_canonical_field(field: &[u8], value: &[u8]) -> bool {
    field.get(..value.len()) == Some(value)
        && field
            .get(value.len()..)
            .is_some_and(|padding| padding.iter().all(|byte| *byte == b' '))
}

fn is_canonical_archive_name(field: &[u8], name: &[u8]) -> bool {
    let Some(slash_index) = name.len().checked_add(1) else {
        return false;
    };
    name.len() < field.len()
        && field.get(..name.len()) == Some(name)
        && field.get(name.len()) == Some(&b'/')
        && field
            .get(slash_index..)
            .is_some_and(|padding| padding.iter().all(|byte| *byte == b' '))
}

fn parse_size(field: &[u8]) -> Option<u64> {
    let value_length = field
        .iter()
        .position(|byte| *byte == b' ')
        .unwrap_or(field.len());
    let (value, padding) = field.split_at(value_length);
    if value.is_empty()
        || !padding.iter().all(|byte| *byte == b' ')
        || (value.len() > 1 && value[0] == b'0')
        || !value.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    value.iter().try_fold(0u64, |number, byte| {
        number.checked_mul(10)?.checked_add(u64::from(*byte - b'0'))
    })
}

fn encoded_member_length(payload_length: u64) -> Option<u64> {
    HEADER_LENGTH
        .checked_add(payload_length)
        .and_then(|length| length.checked_add(payload_length & 1))
}

fn member_name(ordinal: usize) -> Option<[u8; 9]> {
    let mut name = *b"m00000000";
    let mut value = ordinal;
    for digit in name[1..].iter_mut().rev() {
        *digit = b'0' + (value % 10) as u8;
        value /= 10;
    }
    (value == 0).then_some(name)
}

#[cfg(test)]
mod tests;
