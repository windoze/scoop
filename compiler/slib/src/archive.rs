use std::cell::OnceCell;
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use crate::compression::ObjectDecompressor;
use crate::{SlibMember, SlibMemberId, SlibMemberRecord, SlibMemberRole};

pub(crate) const GLOBAL_MAGIC: &[u8; 8] = b"SCOOPSLB";
pub(crate) const HEADER_LENGTH: u64 = 16;

mod index;
pub(crate) use index::{MemberEncoding, MemberPosition, member_positions};

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
    TruncatedPayload {
        member: ArchiveMemberOrdinal,
    },
    NonIncreasingDirectory {
        first_index: usize,
        second_index: usize,
    },
    LengthOverflow,
    Compression {
        member: ArchiveMemberOrdinal,
        source: crate::CompressionError,
    },
}

impl fmt::Display for ArchiveReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("archive global magic is not canonical"),
            Self::TruncatedHeader { member } => {
                write!(formatter, "{member} has a truncated archive header")
            }
            Self::TruncatedPayload { member } => {
                write!(formatter, "{member} payload is truncated")
            }
            Self::NonIncreasingDirectory {
                first_index,
                second_index,
            } => write!(
                formatter,
                "directory member ids {first_index} and {second_index} are not strictly increasing"
            ),
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),
            Self::Compression { member, source } => write!(formatter, "{member}: {source}"),
        }
    }
}

impl std::error::Error for ArchiveReadError {}

/// The manifest frame and the start of the indexed member payloads.
#[derive(Debug, Eq, PartialEq)]
pub struct ManifestArchive<'input> {
    input: &'input [u8],
    manifest: Vec<u8>,
    next_payload: usize,
}

impl<'input> ManifestArchive<'input> {
    pub fn open(input: &'input [u8]) -> Result<Self, ArchiveReadError> {
        let ordinal = ArchiveMemberOrdinal::Manifest;
        let length = read_manifest_header(input)?;
        let next_payload = (HEADER_LENGTH as usize)
            .checked_add(length)
            .ok_or(ArchiveReadError::LengthOverflow)?;
        let manifest = input
            .get(HEADER_LENGTH as usize..next_payload)
            .ok_or(ArchiveReadError::TruncatedPayload { member: ordinal })?;
        let manifest = decompress_member(manifest, ordinal)?;

        Ok(Self {
            input,
            manifest,
            next_payload,
        })
    }

    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    pub fn validate_directory(
        self,
        records: &[SlibMemberRecord],
    ) -> Result<DecodedArchive<'input>, ArchiveReadError> {
        let positions = member_positions(records, self.next_payload as u64)?;
        Ok(DecodedArchive {
            input: self.input,
            manifest: self.manifest,
            members: positions
                .into_iter()
                .map(|(id, position)| (id, position, OnceCell::new()))
                .collect(),
            object_dictionary: records
                .iter()
                .find(|record| matches!(record.role(), SlibMemberRole::ObjectDictionary))
                .map(SlibMemberRecord::id),
            object_decoder: Arc::new(OnceLock::new()),
        })
    }
}

/// Lazily decoded archive members indexed by the manifest directory.
#[derive(Clone, Debug)]
pub struct DecodedArchive<'input> {
    input: &'input [u8],
    manifest: Vec<u8>,
    members: Vec<(SlibMemberId, MemberPosition, OnceCell<MemberPayload>)>,
    object_dictionary: Option<SlibMemberId>,
    object_decoder: Arc<OnceLock<Result<ObjectDecompressor, crate::CompressionError>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MemberPayload {
    Stored(Range<usize>),
    Decoded(Vec<u8>),
}

impl<'input> DecodedArchive<'input> {
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    pub fn member_ids(&self) -> impl ExactSizeIterator<Item = SlibMemberId> + '_ {
        self.members.iter().map(|(id, _, _)| *id)
    }

    pub(crate) fn member(&self, id: SlibMemberId) -> Result<Option<&[u8]>, ArchiveReadError> {
        let Ok(index) = self
            .members
            .binary_search_by_key(&id, |(member, _, _)| *member)
        else {
            return Ok(None);
        };
        let (_, position, cached) = &self.members[index];
        if let Some(bytes) = cached.get() {
            return Ok(Some(self.payload_bytes(bytes)));
        }
        let range = position.range()?;
        let payload = self
            .input
            .get(range.clone())
            .ok_or(ArchiveReadError::TruncatedPayload {
                member: position.ordinal(),
            })?;
        let bytes = match position.encoding {
            MemberEncoding::Zstd => {
                MemberPayload::Decoded(decompress_member(payload, position.ordinal())?)
            }
            MemberEncoding::Object => {
                MemberPayload::Decoded(self.decompress_object(payload, position.ordinal())?)
            }
            MemberEncoding::Stored => MemberPayload::Stored(range),
        };
        Ok(Some(self.payload_bytes(cached.get_or_init(|| bytes))))
    }

    fn decompress_object(
        &self,
        payload: &[u8],
        member: ArchiveMemberOrdinal,
    ) -> Result<Vec<u8>, ArchiveReadError> {
        let decoder = if let Some(decoder) = self.object_decoder.get() {
            decoder
        } else {
            let dictionary = match self.object_dictionary {
                Some(id) => self.member(id)?.expect("indexed dictionary member"),
                None => &[],
            };
            self.object_decoder
                .get_or_init(|| ObjectDecompressor::new(dictionary))
        };
        decoder
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|decoder| decoder.decompress(payload))
            .map_err(|source| ArchiveReadError::Compression { member, source })
    }

    fn payload_bytes<'reader>(&'reader self, payload: &'reader MemberPayload) -> &'reader [u8] {
        match payload {
            MemberPayload::Stored(range) => &self.input[range.clone()],
            MemberPayload::Decoded(bytes) => bytes,
        }
    }
}

pub(crate) fn decompress_member(
    bytes: &[u8],
    member: ArchiveMemberOrdinal,
) -> Result<Vec<u8>, ArchiveReadError> {
    crate::compression::decompress(bytes)
        .map_err(|source| ArchiveReadError::Compression { member, source })
}

pub(crate) fn read_manifest_header(input: &[u8]) -> Result<usize, ArchiveReadError> {
    if input.get(..GLOBAL_MAGIC.len()) != Some(GLOBAL_MAGIC) {
        return Err(ArchiveReadError::BadMagic);
    }
    let length = input
        .get(GLOBAL_MAGIC.len()..HEADER_LENGTH as usize)
        .ok_or(ArchiveReadError::TruncatedHeader {
            member: ArchiveMemberOrdinal::Manifest,
        })?;
    let length = u64::from_le_bytes(length.try_into().expect("eight-byte manifest length"));
    usize::try_from(length).map_err(|_| ArchiveReadError::LengthOverflow)
}

#[cfg(test)]
mod tests;
