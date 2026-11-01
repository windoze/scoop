use super::*;

/// The deterministic indexed byte stream admitted by the `.slib` container
/// schema. Semantic validity of the manifest belongs to the manifest layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSlibArchive {
    bytes: Vec<u8>,
}

impl CanonicalSlibArchive {
    pub fn write(manifest: &[u8], mut members: Vec<SlibMember>) -> Result<Self, ArchiveWriteError> {
        let manifest =
            crate::compression::compress(manifest).map_err(ArchiveWriteError::Compression)?;
        let manifest_length =
            u64::try_from(manifest.len()).map_err(|_| ArchiveWriteError::LengthOverflow)?;

        members.sort_unstable_by_key(|member| member.record().id());
        for pair in members.windows(2) {
            if pair[0].record().id() == pair[1].record().id() {
                return Err(ArchiveWriteError::DuplicateMemberId {
                    id: pair[0].record().id(),
                });
            }
        }

        let mut archive_length = HEADER_LENGTH
            .checked_add(manifest_length)
            .ok_or(ArchiveWriteError::LengthOverflow)?;
        for member in &members {
            let payload_length = u64::try_from(member.payload().len())
                .map_err(|_| ArchiveWriteError::LengthOverflow)?;
            archive_length = archive_length
                .checked_add(payload_length)
                .ok_or(ArchiveWriteError::LengthOverflow)?;
        }

        let capacity =
            usize::try_from(archive_length).map_err(|_| ArchiveWriteError::LengthOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| ArchiveWriteError::Allocation)?;
        output.extend_from_slice(GLOBAL_MAGIC);
        output.extend_from_slice(&manifest_length.to_le_bytes());
        output.extend_from_slice(&manifest);
        for member in members {
            output.extend_from_slice(member.payload());
        }
        debug_assert_eq!(output.len(), capacity);
        Ok(Self { bytes: output })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveWriteError {
    LengthOverflow,

    DuplicateMemberId { id: SlibMemberId },

    Allocation,
    Compression(crate::CompressionError),
}

impl fmt::Display for ArchiveWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),

            Self::DuplicateMemberId { id } => write!(formatter, "duplicate archive member id {id}"),

            Self::Allocation => formatter.write_str("failed to allocate canonical archive bytes"),
            Self::Compression(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArchiveWriteError {}
