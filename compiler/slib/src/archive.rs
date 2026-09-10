use std::fmt;
use std::ops::Range;

use scoop_wire::budget::COLLECTION_ELEMENT_BYTES;
use scoop_wire::{BudgetMeter, WireError, WirePath, sha256};

use crate::{SlibMember, SlibMemberId, SlibMemberRecord};

const GLOBAL_MAGIC: &[u8; 8] = b"!<arch>\n";
const HEADER_LENGTH: u64 = 60;
const MAX_ARCHIVE_BYTES: u64 = 2_147_483_648;
const MAX_MANIFEST_BYTES: u64 = 67_108_864;
const MAX_MEMBERS: usize = 65_536;

/// The unique normal SysV/GNU archive byte stream admitted by the `.slib`
/// container schema. Semantic validity of `manifest.cbor` is established by
/// the manifest layer, not by this physical writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSlibArchive(Vec<u8>);

impl CanonicalSlibArchive {
    pub fn write(manifest: &[u8], mut members: Vec<SlibMember>) -> Result<Self, ArchiveWriteError> {
        let manifest_length =
            u64::try_from(manifest.len()).map_err(|_| ArchiveWriteError::LengthOverflow)?;
        if manifest_length > MAX_MANIFEST_BYTES {
            return Err(ArchiveWriteError::ManifestTooLarge {
                actual: manifest_length,
            });
        }
        if members.len() > MAX_MEMBERS {
            return Err(ArchiveWriteError::TooManyMembers {
                actual: members.len(),
            });
        }

        members.sort_unstable_by_key(|member| member.record().id());
        for pair in members.windows(2) {
            if pair[0].record().id() == pair[1].record().id() {
                return Err(ArchiveWriteError::DuplicateMemberId {
                    id: pair[0].record().id(),
                });
            }
        }

        let mut archive_length =
            u64::try_from(GLOBAL_MAGIC.len()).map_err(|_| ArchiveWriteError::LengthOverflow)?;
        archive_length = archive_length
            .checked_add(
                encoded_member_length(manifest_length).ok_or(ArchiveWriteError::LengthOverflow)?,
            )
            .ok_or(ArchiveWriteError::LengthOverflow)?;
        for member in &members {
            let payload_length = u64::try_from(member.payload().len())
                .map_err(|_| ArchiveWriteError::LengthOverflow)?;
            archive_length = archive_length
                .checked_add(
                    encoded_member_length(payload_length)
                        .ok_or(ArchiveWriteError::LengthOverflow)?,
                )
                .ok_or(ArchiveWriteError::LengthOverflow)?;
        }
        if archive_length > MAX_ARCHIVE_BYTES {
            return Err(ArchiveWriteError::ArchiveTooLarge {
                actual: archive_length,
            });
        }

        let capacity =
            usize::try_from(archive_length).map_err(|_| ArchiveWriteError::LengthOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| ArchiveWriteError::Allocation)?;
        output.extend_from_slice(GLOBAL_MAGIC);
        write_member(&mut output, b"manifest.cbor", manifest)?;
        for (ordinal, member) in members.into_iter().enumerate() {
            let name = member_name(ordinal).ok_or(ArchiveWriteError::TooManyMembers {
                actual: ordinal
                    .checked_add(1)
                    .ok_or(ArchiveWriteError::LengthOverflow)?,
            })?;
            write_member(&mut output, &name, member.payload())?;
        }
        debug_assert_eq!(output.len(), capacity);
        Ok(Self(output))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveWriteError {
    LengthOverflow,
    ManifestTooLarge { actual: u64 },
    TooManyMembers { actual: usize },
    DuplicateMemberId { id: SlibMemberId },
    ArchiveTooLarge { actual: u64 },
    HeaderFieldOverflow,
    Allocation,
}

impl fmt::Display for ArchiveWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),
            Self::ManifestTooLarge { actual } => write!(
                formatter,
                "manifest payload exceeds 67108864 bytes: found {actual}"
            ),
            Self::TooManyMembers { actual } => {
                write!(
                    formatter,
                    "archive exceeds 65536 non-manifest members: found {actual}"
                )
            }
            Self::DuplicateMemberId { id } => write!(formatter, "duplicate archive member id {id}"),
            Self::ArchiveTooLarge { actual } => write!(
                formatter,
                "archive exceeds 2147483648 bytes: found {actual}"
            ),
            Self::HeaderFieldOverflow => {
                formatter.write_str("archive header field exceeds its fixed width")
            }
            Self::Allocation => formatter.write_str("failed to allocate canonical archive bytes"),
        }
    }
}

impl std::error::Error for ArchiveWriteError {}

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
    ArchiveTooLarge {
        actual: u64,
    },
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
    ManifestTooLarge {
        actual: u64,
    },
    TruncatedPayload {
        member: ArchiveMemberOrdinal,
    },
    InvalidPadding {
        member: ArchiveMemberOrdinal,
    },
    TooManyMembers {
        actual: usize,
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
    MemberDigestMismatch {
        id: SlibMemberId,
    },
    LengthOverflow,
    Budget(WireError),
}

impl fmt::Display for ArchiveReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArchiveTooLarge { actual } => write!(
                formatter,
                "archive exceeds 2147483648 bytes: found {actual}"
            ),
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
            Self::ManifestTooLarge { actual } => write!(
                formatter,
                "manifest payload exceeds 67108864 bytes: found {actual}"
            ),
            Self::TruncatedPayload { member } => {
                write!(formatter, "{member} payload is truncated")
            }
            Self::InvalidPadding { member } => {
                write!(formatter, "{member} archive padding is not canonical")
            }
            Self::TooManyMembers { actual } => {
                write!(
                    formatter,
                    "archive exceeds 65536 non-manifest members: found {actual}"
                )
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
            Self::MemberDigestMismatch { id } => {
                write!(
                    formatter,
                    "archive member {id} payload digest does not match its directory record"
                )
            }
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),
            Self::Budget(error) => error.fmt(formatter),
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
    pub fn open(input: &'input [u8], meter: &mut BudgetMeter) -> Result<Self, ArchiveReadError> {
        let input_length =
            u64::try_from(input.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;
        if input_length > MAX_ARCHIVE_BYTES {
            return Err(ArchiveReadError::ArchiveTooLarge {
                actual: input_length,
            });
        }
        let path = WirePath::default();
        meter
            .charge_work(1, &path)
            .map_err(ArchiveReadError::Budget)?;
        if input.get(..GLOBAL_MAGIC.len()) != Some(GLOBAL_MAGIC) {
            return Err(ArchiveReadError::BadMagic);
        }
        meter
            .charge_work(1, &path)
            .map_err(ArchiveReadError::Budget)?;
        let ordinal = ArchiveMemberOrdinal::Manifest;
        let (manifest, next_header) =
            read_member(input, GLOBAL_MAGIC.len(), b"manifest.cbor", ordinal)?;
        let manifest_length =
            u64::try_from(manifest.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;
        if manifest_length > MAX_MANIFEST_BYTES {
            return Err(ArchiveReadError::ManifestTooLarge {
                actual: manifest_length,
            });
        }
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
        meter: &mut BudgetMeter,
    ) -> Result<DecodedArchive<'input>, ArchiveReadError> {
        if records.len() > MAX_MEMBERS {
            return Err(ArchiveReadError::TooManyMembers {
                actual: records.len(),
            });
        }
        let path = WirePath::default();
        let record_count =
            u64::try_from(records.len()).map_err(|_| ArchiveReadError::LengthOverflow)?;
        meter
            .charge_canonical_sequence(record_count, &path)
            .map_err(ArchiveReadError::Budget)?;
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
        meter
            .check_table_entries(record_count, &path)
            .map_err(ArchiveReadError::Budget)?;
        meter
            .try_reserve_exact(&mut ranges, record_count, COLLECTION_ELEMENT_BYTES, &path)
            .map_err(ArchiveReadError::Budget)?;
        let mut cursor = self.next_header;
        for (index, record) in records.iter().enumerate() {
            meter
                .charge_work(1, &path)
                .map_err(ArchiveReadError::Budget)?;
            let ordinal = u32::try_from(index).map_err(|_| ArchiveReadError::LengthOverflow)?;
            let member = ArchiveMemberOrdinal::Directory(ordinal);
            let name = member_name(index).ok_or(ArchiveReadError::TooManyMembers {
                actual: index
                    .checked_add(1)
                    .ok_or(ArchiveReadError::LengthOverflow)?,
            })?;
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
            meter
                .charge_sha256(actual, &path)
                .map_err(ArchiveReadError::Budget)?;
            if sha256(&self.input[range.clone()]) != record.sha256() {
                return Err(ArchiveReadError::MemberDigestMismatch { id: record.id() });
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

/// Canonical physical archive whose member headers, ranges, padding, lengths,
/// and content digests match a typed directory. It intentionally exposes no
/// member payload extraction API or semantic view.
#[derive(Debug, Eq, PartialEq)]
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
    let payload_length_usize =
        usize::try_from(payload_length).map_err(|_| ArchiveReadError::LengthOverflow)?;
    let payload_end = header_end
        .checked_add(payload_length_usize)
        .ok_or(ArchiveReadError::LengthOverflow)?;
    if input.get(header_end..payload_end).is_none() {
        return Err(ArchiveReadError::TruncatedPayload { member });
    }
    let next_header = if payload_length & 1 == 1 {
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
    if ordinal >= MAX_MEMBERS {
        return None;
    }
    let mut name = *b"m00000000";
    let mut value = ordinal;
    for digit in name[1..].iter_mut().rev() {
        *digit = b'0' + (value % 10) as u8;
        value /= 10;
    }
    Some(name)
}

fn write_member(
    output: &mut Vec<u8>,
    name: &[u8],
    payload: &[u8],
) -> Result<(), ArchiveWriteError> {
    let header_start = output.len();
    write_archive_name_field(output, name)?;
    write_field(output, b"0", 12)?;
    write_field(output, b"0", 6)?;
    write_field(output, b"0", 6)?;
    write_field(output, b"100644", 8)?;
    write_field(output, payload.len().to_string().as_bytes(), 10)?;
    output.extend_from_slice(b"`\n");
    debug_assert_eq!(output.len() - header_start, HEADER_LENGTH as usize);
    output.extend_from_slice(payload);
    if payload.len() & 1 == 1 {
        output.push(b'\n');
    }
    Ok(())
}

fn write_archive_name_field(output: &mut Vec<u8>, name: &[u8]) -> Result<(), ArchiveWriteError> {
    let length = name
        .len()
        .checked_add(1)
        .ok_or(ArchiveWriteError::LengthOverflow)?;
    if length > 16 {
        return Err(ArchiveWriteError::HeaderFieldOverflow);
    }
    output.extend_from_slice(name);
    output.push(b'/');
    output.resize(output.len() + 16 - length, b' ');
    Ok(())
}

fn write_field(output: &mut Vec<u8>, value: &[u8], width: usize) -> Result<(), ArchiveWriteError> {
    let padding = width
        .checked_sub(value.len())
        .ok_or(ArchiveWriteError::HeaderFieldOverflow)?;
    output.extend_from_slice(value);
    output.extend(std::iter::repeat_n(b' ', padding));
    Ok(())
}

#[cfg(test)]
mod tests;
