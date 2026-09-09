use std::fmt;

use crate::{SlibMember, SlibMemberId};

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
            .checked_add(encoded_member_length(manifest_length)?)
            .ok_or(ArchiveWriteError::LengthOverflow)?;
        for member in &members {
            let payload_length = u64::try_from(member.payload().len())
                .map_err(|_| ArchiveWriteError::LengthOverflow)?;
            archive_length = archive_length
                .checked_add(encoded_member_length(payload_length)?)
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
        write_member(&mut output, "manifest.cbor", manifest)?;
        for (ordinal, member) in members.into_iter().enumerate() {
            let name = member_name(ordinal)?;
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

fn encoded_member_length(payload_length: u64) -> Result<u64, ArchiveWriteError> {
    HEADER_LENGTH
        .checked_add(payload_length)
        .and_then(|length| length.checked_add(payload_length & 1))
        .ok_or(ArchiveWriteError::LengthOverflow)
}

fn member_name(ordinal: usize) -> Result<String, ArchiveWriteError> {
    if ordinal >= MAX_MEMBERS {
        return Err(ArchiveWriteError::TooManyMembers {
            actual: ordinal.saturating_add(1),
        });
    }
    Ok(format!("m{ordinal:08}"))
}

fn write_member(output: &mut Vec<u8>, name: &str, payload: &[u8]) -> Result<(), ArchiveWriteError> {
    let header_start = output.len();
    let mut archive_name = String::new();
    let archive_name_length = name
        .len()
        .checked_add(1)
        .ok_or(ArchiveWriteError::LengthOverflow)?;
    archive_name
        .try_reserve_exact(archive_name_length)
        .map_err(|_| ArchiveWriteError::Allocation)?;
    archive_name.push_str(name);
    archive_name.push('/');
    write_field(output, archive_name.as_bytes(), 16)?;
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
