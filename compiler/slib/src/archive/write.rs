use super::*;

/// The unique normal SysV/GNU archive byte stream admitted by the `.slib`
/// container schema. Semantic validity of `manifest.cbor` is established by
/// the manifest layer, not by this physical writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSlibArchive {
    bytes: Vec<u8>,
    objects: Vec<(scoop_wire::Digest256, Range<usize>)>,
}

impl CanonicalSlibArchive {
    pub fn write(manifest: &[u8], mut members: Vec<SlibMember>) -> Result<Self, ArchiveWriteError> {
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

        let capacity =
            usize::try_from(archive_length).map_err(|_| ArchiveWriteError::LengthOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| ArchiveWriteError::Allocation)?;
        output.extend_from_slice(GLOBAL_MAGIC);
        write_member(&mut output, b"manifest.cbor", manifest)?;
        let mut objects = Vec::new();
        for (ordinal, member) in members.into_iter().enumerate() {
            let name = member_name(ordinal).ok_or(ArchiveWriteError::HeaderFieldOverflow)?;
            let start = output.len() + HEADER_LENGTH as usize;
            write_member(&mut output, &name, member.payload())?;
            if matches!(
                member.record().role(),
                crate::SlibMemberRole::LinkObject { .. }
            ) {
                objects.push((
                    member.record().sha256(),
                    start..start + member.payload().len(),
                ));
            }
        }
        debug_assert_eq!(output.len(), capacity);
        Ok(Self {
            bytes: output,
            objects,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn object_payloads(&self) -> impl Iterator<Item = (scoop_wire::Digest256, &[u8])> {
        self.objects
            .iter()
            .map(|(digest, range)| (*digest, &self.bytes[range.clone()]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveWriteError {
    LengthOverflow,

    DuplicateMemberId { id: SlibMemberId },

    HeaderFieldOverflow,
    Allocation,
}

impl fmt::Display for ArchiveWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow => formatter.write_str("archive length arithmetic overflowed"),

            Self::DuplicateMemberId { id } => write!(formatter, "duplicate archive member id {id}"),

            Self::HeaderFieldOverflow => {
                formatter.write_str("archive header field exceeds its fixed width")
            }
            Self::Allocation => formatter.write_str("failed to allocate canonical archive bytes"),
        }
    }
}

impl std::error::Error for ArchiveWriteError {}

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
